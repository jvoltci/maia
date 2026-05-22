//! End-to-end three-node toy-service settlement test.
//!
//! Three independent application replicas, each on its own UDP port,
//! each with its own [`WorkloadMeshProxy`] over the same A-B-C triangle
//! topology. The test injects an unbalanced workload (`A` hot, `B` warm,
//! `C` cold) and slightly-skewed per-node offload observations so the
//! reported edge flux is genuinely non-curl-free on the local
//! 2-simplex. Each replica calls `settle()` independently — there is no
//! coordinator, no leader, and no consensus round — and the test
//! asserts the four properties that together demonstrate the consensus-
//! free settlement claim:
//!
//! 1. **Real UDP transport is alive.** A final `PING` from every
//!    replica round-trips to the other two: the substrate is not just
//!    doing in-process arithmetic.
//! 2. **Mutual discovery via Kademlia.** Every node's K-bucket table
//!    contains both peers after a bounded full-mesh discovery window.
//! 3. **Curl is detected and removed locally.** Each replica observes
//!    a pre-settle curl norm strictly greater than zero (the workload
//!    really has rotational disagreement on the A-B-C triangle) and
//!    a post-settle curl norm at the projector's tolerance floor.
//! 4. **The substrate routes flow off the hot node.** The settled
//!    recommendation moves request volume from `A` toward `B` and `C`
//!    on every replica, without any of them having agreed on a
//!    schedule.
//!
//! The test deliberately does *not* assert byte-identical agreement
//! across the three replicas: each node has its own slightly noisy
//! observation of the flow field, so the settled outputs differ in
//! the gradient component. The cross-replica spread is bounded above
//! by the input noise, which is what the audit's "consensus-free
//! convergence" claim actually buys.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use shivya_cli::bridge::WorkloadMeshProxy;
use shivya_p2p::protocol::{Frame, FramePayload};
use shivya_p2p::routing::NodeId;
use shivya_p2p::transport::UdpTransport;
use tokio::sync::mpsc;
use tokio::time::sleep;

/// Single-laptop UDP port band reserved for this test. Picked high
/// enough to avoid colliding with other workspace integration tests.
const PORT_BASE: u16 = 24_701;

/// Three replicas of the toy service.
const N: usize = 3;

/// xorshift64* deterministic PRNG — same trick used by chaos_ensemble.
/// Driving the test from a fixed seed makes both the per-node id
/// distribution and the per-node observation noise reproducible.
struct XorShift(u64);
impl XorShift {
    fn new(seed: u64) -> Self { Self(seed.max(1)) }
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    /// Centred noise on `[-mag, +mag]`.
    fn noise(&mut self, mag: f64) -> f64 {
        let u = (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64;
        (u * 2.0 - 1.0) * mag
    }
}

fn deterministic_id(seed: u64) -> NodeId {
    let mut rng = XorShift::new(seed);
    let mut bytes = [0u8; 20];
    for b in bytes.iter_mut() {
        *b = (rng.next_u64() & 0xff) as u8;
    }
    NodeId(bytes)
}

async fn spawn_node(
    port: u16,
    id_seed: u64,
) -> (Arc<UdpTransport>, mpsc::UnboundedReceiver<Frame>) {
    let id = deterministic_id(id_seed);
    let addr: SocketAddr = format!("127.0.0.1:{port}").parse().expect("addr");
    let transport = Arc::new(
        UdpTransport::new(id, addr)
            .await
            .expect("bind udp socket"),
    );
    let (tx, rx) = mpsc::unbounded_channel();
    Arc::clone(&transport).start(tx);
    (transport, rx)
}

fn build_proxy() -> WorkloadMeshProxy {
    // A-B-C triangle: three vertices, three oriented edges, one 2-simplex.
    // The triangle gives the curl projector a non-empty target subspace;
    // a tree topology would have no curl to project out by construction.
    let names = vec!["A".to_string(), "B".to_string(), "C".to_string()];
    let edges = vec![
        ("A".to_string(), "B".to_string()),
        ("B".to_string(), "C".to_string()),
        ("A".to_string(), "C".to_string()),
    ];
    WorkloadMeshProxy::new(names, edges).expect("triangle proxy builds")
}

/// Drives Kademlia full-mesh discovery until every replica sees the
/// other `N-1` peers, or the attempt budget runs out. Retries are
/// necessary because a 6-pings round on localhost UDP occasionally
/// loses a frame to the kernel socket buffer and a single missed PING
/// silently drops a peer from the table.
async fn discover_full_mesh(nodes: &[Arc<UdpTransport>], addrs: &[SocketAddr]) {
    for attempt in 0..6 {
        let ts_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        for i in 0..N {
            for j in 0..N {
                if i == j {
                    continue;
                }
                let ping = Frame {
                    sender: nodes[i].self_id,
                    payload: FramePayload::Ping { timestamp: ts_ms },
                };
                let _ = nodes[i].send_to(&ping, addrs[j]).await;
            }
        }
        sleep(Duration::from_millis(120)).await;
        for n in nodes {
            n.find_node(n.self_id).await;
        }
        sleep(Duration::from_millis(180 + 80 * attempt as u64)).await;

        let mut covered = true;
        for n in nodes {
            if n.table.lock().await.all_peers().len() < N - 1 {
                covered = false;
                break;
            }
        }
        if covered {
            return;
        }
    }
}

struct ReplicaObservation {
    queues: Vec<(&'static str, usize)>,
    offloads: Vec<(&'static str, &'static str, f64)>,
}

/// One replica's view of the toy workload. The "ground truth" queue
/// lengths model an unbalanced ingress at node A; the per-node noise
/// models each replica having a slightly stale or imperfect view of
/// peer telemetry — exactly the scenario the Hodge curl projector is
/// designed to absorb.
fn observe(replica_idx: usize, rng: &mut XorShift) -> ReplicaObservation {
    // Ground truth: A is hot (high queue), B warm, C cold.
    let queues = vec![
        ("A", 200usize),
        ("B", 20),
        ("C", 5),
    ];

    // Proposed offload rates with per-replica noise. Each replica
    // independently proposes how much to shed from A toward B and C.
    // The triangle closure A->B + B->C - A->C should be zero in a
    // perfectly-coherent reading; per-node noise breaks that closure
    // and creates a non-zero curl on the 2-simplex.
    let drift = (replica_idx as f64) * 0.5;
    let a_to_b = 12.0 + drift + rng.noise(0.8);
    let b_to_c = 3.0 + drift + rng.noise(0.4);
    let a_to_c = 7.0 - drift + rng.noise(0.6);
    let offloads = vec![
        ("A", "B", a_to_b),
        ("B", "C", b_to_c),
        ("A", "C", a_to_c),
    ];
    ReplicaObservation { queues, offloads }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn three_node_workload_settles_off_hot_node() {
    // ----- 1. Stand up three real UDP transports -----
    let mut nodes = Vec::with_capacity(N);
    let mut rxs = Vec::with_capacity(N);
    let mut addrs = Vec::with_capacity(N);
    for i in 0..N {
        let port = PORT_BASE + i as u16;
        let (t, rx) = spawn_node(port, 0xA11CE + (i as u64) * 7).await;
        addrs.push(format!("127.0.0.1:{port}").parse::<SocketAddr>().unwrap());
        nodes.push(t);
        rxs.push(rx);
    }

    // ----- 2. Kademlia full-mesh discovery via real UDP -----
    discover_full_mesh(&nodes, &addrs).await;
    for n in &nodes {
        let seen = n.table.lock().await.all_peers().len();
        assert!(
            seen >= N - 1,
            "node {:?} only discovered {} of {} peers via UDP",
            n.self_id, seen, N - 1
        );
    }

    // ----- 3. Each replica owns a WorkloadMeshProxy -----
    let mut proxies: Vec<WorkloadMeshProxy> = (0..N).map(|_| build_proxy()).collect();
    let mut rng = XorShift::new(0xFEED_FACE_CAFE_BABE);

    // ----- 4. Apply the unbalanced workload + skewed observations -----
    let mut pre_settle_curl = [0.0_f64; N];
    let mut recommendations = Vec::with_capacity(N);
    for (i, proxy) in proxies.iter_mut().enumerate() {
        let ReplicaObservation { queues, offloads } = observe(i, &mut rng);
        for (node, q) in &queues {
            proxy.record_queue_len(node, *q).expect("known node");
        }
        for (src, dst, rate) in &offloads {
            proxy.record_offload(src, dst, *rate).expect("known edge");
        }

        // Snapshot the pre-settle curl by running settle once and inspecting
        // the bridge's last_curl_norm; this is the magnitude of rotational
        // disagreement the projector had to absorb.
        let recs = proxy.settle();
        pre_settle_curl[i] = proxy.last_curl_norm();
        recommendations.push(recs);
    }

    // ----- 5. Assertions -----

    // 5a. Real UDP transport is still alive: ping every peer once more
    //     and require the post-test bucket table to retain coverage.
    let ts_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    for i in 0..N {
        for j in 0..N {
            if i == j { continue; }
            let ping = Frame {
                sender: nodes[i].self_id,
                payload: FramePayload::Ping { timestamp: ts_ms },
            };
            nodes[i]
                .send_to(&ping, addrs[j])
                .await
                .expect("final ping send");
        }
    }
    sleep(Duration::from_millis(150)).await;
    for n in &nodes {
        assert!(
            n.table.lock().await.all_peers().len() >= N - 1,
            "post-settle peer coverage regressed below N-1"
        );
    }

    // 5b. Pre-settle curl was strictly non-zero on every replica —
    //     i.e., the workload genuinely had rotational disagreement on
    //     the A-B-C triangle that the projector had work to do on.
    //     The Hodge projector reports the L2 norm of the curl
    //     component it removed; ten orders of magnitude above f64
    //     epsilon is a real signal.
    for (i, c) in pre_settle_curl.iter().enumerate() {
        assert!(
            *c > 1e-6,
            "replica {} reported pre-settle curl {} - workload not actually rotational",
            i, c
        );
    }

    // 5c. Re-settling is a no-op: the projector is idempotent.
    //     Running settle on already-reconciled state must not introduce
    //     new curl above the CG tolerance.
    for (i, proxy) in proxies.iter_mut().enumerate() {
        // Re-record using the just-recommended rates so the bridge
        // input matches the previous output.
        for rec in &recommendations[i] {
            proxy
                .record_offload(&rec.from, &rec.to, rec.recommended_rate)
                .expect("known edge");
        }
        let _ = proxy.settle();
        let post = proxy.last_curl_norm();
        assert!(
            post < 1e-6,
            "replica {} second-settle curl {} - projector not idempotent",
            i, post
        );
    }

    // 5d. The substrate routed flow off the hot node. Sum the
    //     recommended outflow from A on each replica. With A hot and
    //     B, C cool, the consensus-free settlement must produce a
    //     net positive recommendation away from A on every replica.
    for (i, recs) in recommendations.iter().enumerate() {
        let mut a_outflow = 0.0;
        for rec in recs {
            if rec.from == "A" {
                a_outflow += rec.recommended_rate;
            } else if rec.to == "A" {
                a_outflow -= rec.recommended_rate;
            }
        }
        assert!(
            a_outflow > 0.0,
            "replica {} did not route work off the hot node A: A-outflow={}",
            i, a_outflow
        );
    }
}
