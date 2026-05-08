//! Print one snapshot of both collectors as JSON-ish text. Useful for smoke-testing
//! without a TTY (the main TUI requires a real terminal).
//!
//! Run: cargo run --example probe -- [--ollama-url http://127.0.0.1:11434]

use llmtop::collectors;

#[tokio::main]
async fn main() {
    let url = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "http://127.0.0.1:11434".into());

    let hw = collectors::poll_hardware().await;
    println!("=== HARDWARE ===");
    println!("gpu_name      = {:?}", hw.gpu_name);
    println!("gpu_util_pct  = {}", hw.gpu_util_pct);
    println!("vram_used_mb  = {}", hw.vram_used_mb);
    println!("vram_total_mb = {}", hw.vram_total_mb);
    println!("power_w       = {:.2}", hw.power_w);
    println!("power_limit_w = {:.2}", hw.power_limit_w);
    println!("temp_c        = {}", hw.temp_c);

    let models = collectors::poll_ollama(&url).await;
    println!("\n=== OLLAMA models ===");
    if models.is_empty() {
        println!("(none — ollama not running or no model loaded)");
    }
    for m in models {
        println!(
            "name={:<28} vram_mb={:>6} tok/s={:>5.1}",
            m.name, m.vram_mb, m.tokens_per_sec
        );
    }
}
