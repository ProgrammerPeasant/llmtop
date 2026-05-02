//! Pricing tables + USD-equivalent computation. Filled in Day 6-7.

pub mod co2;

/// $/1M tokens for output (rough — update via PRs).
pub fn provider_output_per_million(provider: &str) -> Option<f64> {
    match provider {
        "claude-sonnet" => Some(15.0),
        "gpt-4o" => Some(10.0),
        "gemini-2.5" => Some(10.0),
        _ => None,
    }
}
