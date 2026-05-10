//! Ollama collector. Polls /api/ps for loaded models + VRAM. Tokens/sec
//! data is filled in from the proxy sink (when `--proxy` is enabled);
//! without the proxy `tokens_per_sec` stays 0 and the model row shows `idle`.

use crate::{collectors::ModelInfo, proxy::Sink};
use serde::Deserialize;
use std::time::{Duration, Instant};

#[derive(Debug, Deserialize)]
struct PsResponse {
    models: Vec<PsModel>,
}

#[derive(Debug, Deserialize)]
struct PsModel {
    name: String,
    #[serde(default)]
    size_vram: u64,
}

/// Tokens/sec is treated as 0 if no proxy sample arrived in this window —
/// the model is loaded but idle. Generous so a slow generation still counts.
const TOK_FRESH: Duration = Duration::from_secs(5);

pub async fn poll(base_url: &str, sink: Option<&Sink>) -> Vec<ModelInfo> {
    let url = format!("{}/api/ps", base_url.trim_end_matches('/'));
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_millis(800))
        .build()
    {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    let resp = match client.get(&url).send().await {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    let parsed: PsResponse = match resp.json().await {
        Ok(p) => p,
        Err(_) => return Vec::new(),
    };

    let now = Instant::now();
    parsed
        .models
        .into_iter()
        .map(|m| {
            let (tok_s, total) = sink
                .and_then(|s| s.lock().ok())
                .and_then(|map| map.get(&m.name).cloned())
                .map(|stats| {
                    let fresh = stats
                        .last_seen
                        .is_some_and(|t| now.duration_since(t) <= TOK_FRESH);
                    (if fresh { stats.last_tok_s } else { 0.0 }, stats.total_tokens)
                })
                .unwrap_or((0.0, 0));
            ModelInfo {
                name: m.name,
                vram_mb: m.size_vram / 1024 / 1024,
                tokens_per_sec: tok_s,
                total_tokens: total,
            }
        })
        .collect()
}
