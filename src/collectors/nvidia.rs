//! NVIDIA collector via nvml-wrapper. Lazy init — gracefully no-ops without CUDA/NVML.
//!
//! Multi-GPU: v0.1 aggregates all devices into one snapshot (sum power/VRAM, avg util,
//! max temp). Per-GPU breakdown view planned for v0.2 (`--per-gpu` flag).

#[cfg(feature = "nvidia")]
mod imp {
    use crate::collectors::HardwareSnapshot;
    use nvml_wrapper::Nvml;
    use std::sync::OnceLock;

    static NVML: OnceLock<Option<Nvml>> = OnceLock::new();

    fn nvml() -> Option<&'static Nvml> {
        NVML.get_or_init(|| Nvml::init().ok()).as_ref()
    }

    pub fn poll() -> Option<HardwareSnapshot> {
        let nvml = nvml()?;
        let count = nvml.device_count().ok()?;
        if count == 0 {
            return None;
        }

        let mut names: Vec<String> = Vec::with_capacity(count as usize);
        let mut util_sum: u64 = 0;
        let mut util_n: u64 = 0;
        let mut vram_total: u64 = 0;
        let mut vram_used: u64 = 0;
        let mut power_mw: u64 = 0;
        let mut limit_mw: u64 = 0;
        let mut temp_max: u32 = 0;

        for i in 0..count {
            let Ok(dev) = nvml.device_by_index(i) else { continue };

            names.push(dev.name().unwrap_or_else(|_| "NVIDIA GPU".into()));
            if let Ok(u) = dev.utilization_rates() {
                util_sum += u.gpu as u64;
                util_n += 1;
            }
            if let Ok(m) = dev.memory_info() {
                vram_total += m.total / 1024 / 1024;
                vram_used += m.used / 1024 / 1024;
            }
            power_mw += dev.power_usage().unwrap_or(0) as u64;
            limit_mw += dev.enforced_power_limit().unwrap_or(0) as u64;
            if let Ok(t) =
                dev.temperature(nvml_wrapper::enum_wrappers::device::TemperatureSensor::Gpu)
            {
                temp_max = temp_max.max(t);
            }
        }

        let gpu_name = aggregate_name(&names);
        let util_avg = util_sum.checked_div(util_n).unwrap_or(0) as u32;

        Some(HardwareSnapshot {
            gpu_name,
            gpu_count: count,
            gpu_util_pct: util_avg,
            vram_total_mb: vram_total,
            vram_used_mb: vram_used,
            power_w: power_mw as f64 / 1000.0,
            power_limit_w: limit_mw as f64 / 1000.0,
            temp_c: temp_max,
        })
    }

    fn aggregate_name(names: &[String]) -> String {
        match names.len() {
            0 => "NVIDIA GPU".into(),
            1 => names[0].clone(),
            n => {
                let first = &names[0];
                if names.iter().all(|x| x == first) {
                    format!("{}× {}", n, first)
                } else {
                    format!("{}× GPU (mixed)", n)
                }
            }
        }
    }
}

#[cfg(not(feature = "nvidia"))]
mod imp {
    use crate::collectors::HardwareSnapshot;
    pub fn poll() -> Option<HardwareSnapshot> {
        None
    }
}

pub use imp::poll;
