mod telemetry;
mod orchestrator;

use clap::{Parser, Subcommand};
use std::sync::Arc;
use tokio::sync::Mutex;
use std::path::Path;
use tokio::net::UnixListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use telemetry::TelemetrySampler;
use orchestrator::NativeOrchestrator;

const SOCKET_PATH: &str = "/tmp/shivya_cli.sock";

#[derive(Parser)]
#[command(name = "shivya-cli")]
#[command(about = "Shivya Headless Daemon CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the background daemon node
    Start {
        /// Spawn the orchestrator in the background
        #[arg(long)]
        daemon: bool,
    },
    /// Query active memory segments and print metrics
    Status,
}

#[cfg(unix)]
async fn wait_for_signals() {
    use tokio::signal::unix::{signal, SignalKind};
    let mut sigint = signal(SignalKind::interrupt()).expect("Failed to bind SIGINT listener");
    let mut sigterm = signal(SignalKind::terminate()).expect("Failed to bind SIGTERM listener");
    tokio::select! {
        _ = sigint.recv() => {
            println!("\n[Apoptosis] Received SIGINT. Running orderly apoptotic memory teardown...");
        }
        _ = sigterm.recv() => {
            println!("\n[Apoptosis] Received SIGTERM. Running orderly apoptotic memory teardown...");
        }
    }
}

#[cfg(not(unix))]
async fn wait_for_signals() {
    tokio::signal::ctrl_c().await.expect("Failed to bind Ctrl-C listener");
    println!("\n[Apoptosis] Received Ctrl-C. Running orderly apoptotic memory teardown...");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Cli::parse();

    match args.command {
        Commands::Start { daemon } => {
            println!("Initializing Shivya 5-Layer Edge Daemon...");

            if daemon {
                println!("[Daemon] Spawning unified orchestration engine in dedicated background thread pool.");
            }

            // Stale socket cleanup step on initialization
            if Path::new(SOCKET_PATH).exists() {
                println!("[UDS] Lingering stale socket file found. Performing clean-up.");
                let _ = std::fs::remove_file(SOCKET_PATH);
            }

            let orchestrator = Arc::new(Mutex::new(NativeOrchestrator::new(10)));
            let orchestrator_clone = Arc::clone(&orchestrator);

            // Telemetry and scheduling task loop (runs every 1000ms)
            tokio::spawn(async move {
                let mut sampler = TelemetrySampler::new();
                let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(1000));
                loop {
                    interval.tick().await;
                    let (cpu, rx, tx) = sampler.sample();
                    let net_rate = (rx + tx) as f64;
                    let mut orch = orchestrator_clone.lock().await;
                    orch.step(cpu as f64, net_rate);
                }
            });

            // UDS Listener Task for Status Query requests
            let listener = UnixListener::bind(SOCKET_PATH)?;
            let orchestrator_uds = Arc::clone(&orchestrator);
            tokio::spawn(async move {
                while let Ok((mut stream, _)) = listener.accept().await {
                    let response = {
                        let orch = orchestrator_uds.lock().await;
                        orch.get_status_json()
                    };
                    let _ = stream.write_all(response.as_bytes()).await;
                }
            });

            println!("[Lifecycle] Node running. UDS listener bound to {}", SOCKET_PATH);
            
            // Wait for termination signal
            wait_for_signals().await;

            // Apoptotic cleanups before exit
            if Path::new(SOCKET_PATH).exists() {
                let _ = std::fs::remove_file(SOCKET_PATH);
            }
            println!("[Lifecycle] Apoptotic clean-up complete. Node gracefully terminated.");
        }
        Commands::Status => {
            if !Path::new(SOCKET_PATH).exists() {
                eprintln!("Error: Shivya daemon socket not found at {}. Is the daemon running?", SOCKET_PATH);
                std::process::exit(1);
            }

            let mut stream = tokio::net::UnixStream::connect(SOCKET_PATH).await?;
            let mut response = Vec::new();
            stream.read_to_end(&mut response).await?;

            let json_str = String::from_utf8_lossy(&response);
            
            // Parse JSON for formatted printout
            match serde_json::from_str::<serde_json::Value>(&json_str) {
                Ok(status) => {
                    println!("============================================================");
                    println!("             SHIVYA HEADLESS DAEMON STATUS REGISTRY         ");
                    println!("============================================================");
                    println!("Collective Free Energy Level  : {:.4}", status["collective_free_energy"].as_f64().unwrap_or(0.0));
                    println!("Topological Curl Deviation    : {:.4}", status["curl_deviation"].as_f64().unwrap_or(0.0));
                    println!("Active Node Count             : {}", status["active_nodes_count"]);
                    println!("Morphogenetic Active Pool     : {:?}", status["active_pool"]);
                    println!("------------------------------------------------------------");
                    println!("Node Memory Details:");
                    if let Some(nodes) = status["nodes"].as_array() {
                        for node in nodes {
                            if node["active"].as_bool().unwrap_or(false) {
                                println!("  Node #{} (ACTIVE):", node["id"]);
                                println!("    - Free Energy              : {:.4}", node["free_energy"].as_f64().unwrap_or(0.0));
                                println!("    - Belief Dimensions        : {}", node["belief_dim"]);
                                println!("    - Morphic Instruction Count: {} insts", node["instruction_count"]);
                                println!("    - Turing Morphogen (U / V) : {:.4} / {:.4}", 
                                    node["morphogen_u"].as_f64().unwrap_or(0.0),
                                    node["morphogen_v"].as_f64().unwrap_or(0.0)
                                );
                                println!("    - Morphic AST Equation     : {}", node["morphic_equation"]);
                            }
                        }
                    }
                    println!("============================================================");
                }
                Err(_) => {
                    // Fallback to raw json if parse fails
                    println!("{}", json_str);
                }
            }
        }
    }

    Ok(())
}
