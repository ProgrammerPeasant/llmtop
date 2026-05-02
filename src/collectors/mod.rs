//! Collector layer. Day 1 = stub data so TUI renders.
//! Real implementations land in nvidia.rs / apple.rs / ollama.rs in following days.

pub mod nvidia;
pub mod apple;
pub mod ollama;

#[derive(Debug, Clone, Default)]
pub struct HardwareSnapshot {
    pub gpu_name: String,
    pub gpu_util_pct: u32,
    pub vram_total_mb: u64,
    pub vram_used_mb: u64,
    pub power_w: f64,
    pub power_limit_w: f64,
    pub temp_c: u32,
}

#[derive(Debug, Clone)]
pub struct ModelInfo {
    pub name: String,
    pub vram_mb: u64,
    pub tokens_per_sec: f64,
    /// Monotonic counter — used to derive per-tick deltas for session totals.
    pub total_tokens: u64,
}

/// Hardware poll — tries NVIDIA, then Apple. Returns default when no backend present.
pub async fn poll_hardware() -> HardwareSnapshot {
    if let Some(s) = nvidia::poll() {
        return s;
    }
    #[cfg(target_os = "macos")]
    if let Some(s) = apple::poll() {
        return s;
    }
    HardwareSnapshot::default()
}

pub async fn poll_ollama(base_url: &str) -> Vec<ModelInfo> {
    ollama::poll(base_url).await
}
