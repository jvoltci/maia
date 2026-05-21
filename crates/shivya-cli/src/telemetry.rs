use sysinfo::{System, Networks};

#[derive(Debug, Clone, Copy)]
pub struct HostSnapshot {
    pub cpu: f32,
    pub net_rx: u64,
    pub net_tx: u64,
    pub memory_used_ratio: f64,
}

pub struct TelemetrySampler {
    sys: System,
    networks: Networks,
}

impl TelemetrySampler {
    pub fn new() -> Self {
        let mut sys = System::new();
        sys.refresh_cpu_usage();
        sys.refresh_memory();
        let networks = Networks::new_with_refreshed_list();
        Self { sys, networks }
    }

    pub fn sample(&mut self) -> HostSnapshot {
        self.sys.refresh_cpu_usage();
        self.sys.refresh_memory();
        let cpu = self.sys.global_cpu_info().cpu_usage();

        let total_mem = self.sys.total_memory();
        let used_mem = self.sys.used_memory();
        let memory_used_ratio = if total_mem > 0 {
            used_mem as f64 / total_mem as f64
        } else {
            0.0
        };

        self.networks.refresh();
        let mut net_rx = 0;
        let mut net_tx = 0;
        for (_name, data) in &self.networks {
            net_rx += data.received();
            net_tx += data.transmitted();
        }

        HostSnapshot { cpu, net_rx, net_tx, memory_used_ratio }
    }
}
