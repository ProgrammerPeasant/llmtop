//! NVIDIA collector via nvml-wrapper. Lazy init — gracefully no-ops without CUDA/NVML.

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
        let dev = nvml.device_by_index(0).ok()?;

        let name = dev.name().unwrap_or_else(|_| "NVIDIA GPU".into());
        let util = dev.utilization_rates().ok().map(|u| u.gpu).unwrap_or(0);
        let mem = dev.memory_info().ok();
        let (vram_total, vram_used) = mem
            .map(|m| (m.total / 1024 / 1024, m.used / 1024 / 1024))
            .unwrap_or((0, 0));
        // power_usage returns milliwatts.
        let power_mw = dev.power_usage().unwrap_or(0);
        let limit_mw = dev.enforced_power_limit().unwrap_or(0);
        let temp = dev
            .temperature(nvml_wrapper::enum_wrappers::device::TemperatureSensor::Gpu)
            .unwrap_or(0);

        Some(HardwareSnapshot {
            gpu_name: name,
            gpu_util_pct: util,
            vram_total_mb: vram_total,
            vram_used_mb: vram_used,
            power_w: power_mw as f64 / 1000.0,
            power_limit_w: limit_mw as f64 / 1000.0,
            temp_c: temp,
        })
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
