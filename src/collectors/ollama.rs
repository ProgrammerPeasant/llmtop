//! Ollama collector. MVP: poll /api/ps for loaded models + VRAM.
//! Tokens/sec via log tailing comes Day 5.

use crate::collectors::ModelInfo;
use serde::Deserialize;
use std::time::Duration;

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

pub async fn poll(base_url: &str) -> Vec<ModelInfo> {
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
    parsed
        .models
        .into_iter()
        .map(|m| ModelInfo {
            name: m.name,
            vram_mb: m.size_vram / 1024 / 1024,
            tokens_per_sec: 0.0,
            total_tokens: 0,
        })
        .collect()
}
