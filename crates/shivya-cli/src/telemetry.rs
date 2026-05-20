use sysinfo::{System, Networks};

pub struct TelemetrySampler {
    sys: System,
    networks: Networks,
}

impl TelemetrySampler {
    pub fn new() -> Self {
        let mut sys = System::new();
        // Initial refresh to populate CPU statistics
        sys.refresh_cpu_usage();
        let networks = Networks::new_with_refreshed_list();
        Self { sys, networks }
    }

    pub fn sample(&mut self) -> (f32, u64, u64) {
        // Refresh CPU
        self.sys.refresh_cpu_usage();
        let cpu_usage = self.sys.global_cpu_info().cpu_usage();

        // Refresh Networks
        self.networks.refresh();
        let mut total_rx = 0;
        let mut total_tx = 0;
        for (_name, data) in &self.networks {
            total_rx += data.received();
            total_tx += data.transmitted();
        }

        (cpu_usage, total_rx, total_tx)
    }
}
