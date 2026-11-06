//! llama.cpp collector (llama-server).
//!
//! - Model name(s): `GET /v1/models` (OpenAI-style `data[].id`), falling back
//!   to `GET /props` (`model_alias`, else `model_path`). llama-server uses the
//!   full GGUF path as the id by default, so `*.gguf` paths are shortened to
//!   the file stem for display.
//! - Throughput: `GET /metrics` (Prometheus text, only served when
//!   llama-server was started with `--metrics`). `llamacpp:tokens_predicted_total`
//!   is monotonic but only increments when a request *finishes*, so on its own
//!   it reads 0 tok/s during generation and then spikes. To get a live rate we
//!   add the in-flight `n_decoded` of busy slots from `GET /slots` (on by
//!   default; disabled with `--no-slots`). Without `/slots`, we fall back to
//!   dividing by `llamacpp:tokens_predicted_seconds_total` growth, which gives
//!   a correct (if per-request, one-tick) rate when a request completes.
//! - VRAM: llama-server does not expose it, so `vram_mb` is always 0.
//! - While the model is loading every endpoint returns 503, so poll() returns
//!   an empty Vec until it's ready.
//!
//! Limitation: `/metrics` and `/slots` are server-wide, not per-model. When the
//! server lists several models (router mode), all metrics are attributed to
//! the first model and the others report 0.

use super::ModelInfo;
use serde::Deserialize;
use std::time::{Duration, Instant};

const TIMEOUT: Duration = Duration::from_millis(800);
const TOKENS_PREDICTED: &str = "llamacpp:tokens_predicted_total";
const TOKENS_PREDICTED_SECONDS: &str = "llamacpp:tokens_predicted_seconds_total";

pub struct LlamaCpp {
    base_url: String,
    client: Option<reqwest::Client>,
    rate: RateTracker,
}

impl LlamaCpp {
    pub fn new(base_url: String) -> Self {
        let client = reqwest::Client::builder().timeout(TIMEOUT).build().ok();
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            client,
            rate: RateTracker::default(),
        }
    }

    /// Poll llama-server once. Returns loaded model(s); empty Vec if unreachable.
    pub async fn poll(&mut self) -> Vec<ModelInfo> {
        let Some(client) = self.client.clone() else {
            return Vec::new();
        };

        let mut names = self.fetch_models(&client).await;
        if names.is_empty() {
            names = self.fetch_props_name(&client).await.into_iter().collect();
        }
        if names.is_empty() {
            // Unreachable, loading, or not a llama-server. Drop the baseline so
            // a later reconnect doesn't compute a rate across the gap.
            self.rate = RateTracker::default();
            return Vec::new();
        }

        // Metrics strictly before slots: if a request finishes between the two
        // fetches we under-count for one tick (clamped by RateTracker) rather
        // than double-count.
        let metrics = self.fetch_metrics(&client).await;
        let slots = self.fetch_slots(&client).await;
        let ctx_size = match slots.as_deref().and_then(ctx_from_slots) {
            Some(n) => Some(n),
            None => self.fetch_props_ctx(&client).await,
        };
        let (tok_s, total) = match metrics.as_deref().and_then(counters_from_metrics) {
            Some((counter, gen_secs)) => {
                let sample = CounterSample {
                    finished: counter,
                    gen_seconds: gen_secs,
                    in_flight: slots.as_deref().map(in_flight_tokens),
                };
                self.rate.update(sample, Instant::now())
            }
            // /metrics off, or a transient failure/timeout: keep the last known
            // total. Reporting 0 here would become the app's session baseline
            // and the next good read would count the server's lifetime tokens.
            None => (0.0, self.rate.last_total().unwrap_or(0)),
        };

        names
            .into_iter()
            .enumerate()
            .map(|(i, name)| {
                let (tokens_per_sec, total_tokens) = if i == 0 { (tok_s, total) } else { (0.0, 0) };
                ModelInfo {
                    name,
                    vram_mb: 0,
                    tokens_per_sec,
                    total_tokens,
                    ctx_size: if i == 0 { ctx_size } else { None },
                }
            })
            .collect()
    }

    async fn get_ok(&self, client: &reqwest::Client, path: &str) -> Option<reqwest::Response> {
        let resp = client
            .get(format!("{}{}", self.base_url, path))
            .send()
            .await
            .ok()?;
        resp.status().is_success().then_some(resp)
    }

    async fn fetch_models(&self, client: &reqwest::Client) -> Vec<String> {
        let Some(resp) = self.get_ok(client, "/v1/models").await else {
            return Vec::new();
        };
        resp.json::<ModelsResponse>()
            .await
            .map(models_from_response)
            .unwrap_or_default()
    }

    async fn fetch_props_name(&self, client: &reqwest::Client) -> Option<String> {
        let props: PropsResponse = self.get_ok(client, "/props").await?.json().await.ok()?;
        name_from_props(&props)
    }

    async fn fetch_metrics(&self, client: &reqwest::Client) -> Option<String> {
        // 404/501 when --metrics is off -> None.
        self.get_ok(client, "/metrics").await?.text().await.ok()
    }

    /// `/slots` (on by default; None with `--no-slots`). Fetched strictly after
    /// `/metrics`: if a request finishes between the two we under-count for one
    /// tick (clamped by RateTracker) rather than double-count.
    async fn fetch_slots(&self, client: &reqwest::Client) -> Option<Vec<Slot>> {
        self.get_ok(client, "/slots").await?.json().await.ok()
    }

    async fn fetch_props_ctx(&self, client: &reqwest::Client) -> Option<u64> {
        let props: PropsResponse = self.get_ok(client, "/props").await?.json().await.ok()?;
        props.default_generation_settings?.n_ctx.filter(|&n| n > 0)
    }
}

#[derive(Debug, Deserialize)]
struct ModelsResponse {
    #[serde(default)]
    data: Vec<ModelEntry>,
}

#[derive(Debug, Deserialize)]
struct ModelEntry {
    id: String,
}

#[derive(Debug, Default, Deserialize)]
struct PropsResponse {
    #[serde(default)]
    model_alias: Option<String>,
    #[serde(default)]
    model_path: Option<String>,
    #[serde(default)]
    default_generation_settings: Option<GenerationSettings>,
}

#[derive(Debug, Default, Deserialize)]
struct GenerationSettings {
    #[serde(default)]
    n_ctx: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct Slot {
    #[serde(default)]
    is_processing: bool,
    /// Per-slot context window (each parallel slot gets its own).
    #[serde(default)]
    n_ctx: Option<u64>,
    /// Array of one entry in current llama-server; a bare object in older builds.
    #[serde(default)]
    next_token: Option<serde_json::Value>,
}

/// Context window per request: every slot has the same `n_ctx`, so take the first.
fn ctx_from_slots(slots: &[Slot]) -> Option<u64> {
    slots.iter().find_map(|s| s.n_ctx).filter(|&n| n > 0)
}

fn in_flight_tokens(slots: &[Slot]) -> u64 {
    slots
        .iter()
        .filter(|s| s.is_processing)
        .filter_map(|s| {
            let nt = s.next_token.as_ref()?;
            let obj = nt.as_array().and_then(|a| a.first()).unwrap_or(nt);
            obj.get("n_decoded")?.as_u64()
        })
        .sum()
}

/// `(tokens_predicted_total, tokens_predicted_seconds_total)` from /metrics text.
fn counters_from_metrics(text: &str) -> Option<(u64, f64)> {
    let tokens = metric_value(text, TOKENS_PREDICTED).filter(|v| v.is_finite() && *v >= 0.0)?;
    let secs = metric_value(text, TOKENS_PREDICTED_SECONDS)
        .filter(|v| v.is_finite() && *v >= 0.0)
        .unwrap_or(0.0);
    Some((tokens as u64, secs))
}

fn models_from_response(r: ModelsResponse) -> Vec<String> {
    r.data
        .into_iter()
        .map(|m| display_name(&m.id))
        .filter(|id| !id.is_empty())
        .collect()
}

fn name_from_props(p: &PropsResponse) -> Option<String> {
    [p.model_alias.as_deref(), p.model_path.as_deref()]
        .into_iter()
        .flatten()
        .map(display_name)
        .find(|s| !s.is_empty())
}

/// `/mnt/models/foo-Q4_K_M.gguf` -> `foo-Q4_K_M`. Non-path ids (aliases like
/// `org/model`) are kept as-is.
fn display_name(id: &str) -> String {
    let id = id.trim();
    let lower = id.to_ascii_lowercase();
    if !lower.ends_with(".gguf") {
        return id.to_string();
    }
    let base = id.rsplit(['/', '\\']).next().unwrap_or(id);
    base[..base.len() - ".gguf".len()].to_string()
}

/// One Prometheus text-format sample.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Sample<'a> {
    pub name: &'a str,
    /// Raw label body between `{` and `}` (unparsed), empty if none.
    pub labels: &'a str,
    pub value: f64,
}

/// Minimal Prometheus text exposition parser. Skips comments/blank lines and
/// anything malformed. Optional trailing timestamp is ignored.
pub(crate) fn parse_prometheus(text: &str) -> Vec<Sample<'_>> {
    text.lines().filter_map(parse_line).collect()
}

fn parse_line(line: &str) -> Option<Sample<'_>> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let name_end = line
        .find(|c: char| c == '{' || c.is_whitespace())
        .unwrap_or(line.len());
    let name = &line[..name_end];
    if name.is_empty() {
        return None;
    }
    let mut rest = &line[name_end..];
    let mut labels = "";
    if let Some(after) = rest.strip_prefix('{') {
        let close = find_label_close(after)?;
        labels = &after[..close];
        rest = &after[close + 1..];
    }
    let value = parse_value(rest.split_whitespace().next()?)?;
    Some(Sample {
        name,
        labels,
        value,
    })
}

/// Index of the `}` closing a label set, respecting quoted values.
fn find_label_close(s: &str) -> Option<usize> {
    let mut in_quotes = false;
    let mut escaped = false;
    for (i, c) in s.char_indices() {
        match c {
            _ if escaped => escaped = false,
            '\\' if in_quotes => escaped = true,
            '"' => in_quotes = !in_quotes,
            '}' if !in_quotes => return Some(i),
            _ => {}
        }
    }
    None
}

fn parse_value(s: &str) -> Option<f64> {
    match s {
        "+Inf" => Some(f64::INFINITY),
        "-Inf" => Some(f64::NEG_INFINITY),
        _ => s.parse().ok(),
    }
}

/// First sample value for `name` (any labels).
pub(crate) fn metric_value(text: &str, name: &str) -> Option<f64> {
    parse_prometheus(text)
        .into_iter()
        .find(|s| s.name == name)
        .map(|s| s.value)
}

/// One poll's worth of throughput counters.
#[derive(Debug, Clone, Copy)]
pub(crate) struct CounterSample {
    /// `llamacpp:tokens_predicted_total` (finished requests only).
    pub finished: u64,
    /// `llamacpp:tokens_predicted_seconds_total`.
    pub gen_seconds: f64,
    /// Sum of `n_decoded` over busy slots; None if `/slots` is unavailable.
    pub in_flight: Option<u64>,
}

/// Derives tokens/sec from successive counter samples.
#[derive(Debug, Default)]
pub(crate) struct RateTracker {
    prev: Option<Prev>,
}

#[derive(Debug, Clone, Copy)]
struct Prev {
    finished: u64,
    gen_seconds: f64,
    live: u64,
    at: Instant,
}

impl RateTracker {
    /// Monotonic total from the most recent sample, if any.
    pub fn last_total(&self) -> Option<u64> {
        self.prev.map(|p| p.live)
    }

    /// Record a sample; returns `(tokens_per_sec, monotonic_total)`.
    ///
    /// First sample, unchanged counter, or zero elapsed time give 0 tok/s.
    /// If the finished-request counter went *down* the server restarted: the
    /// baseline is reset and 0 is returned (never negative / huge).
    pub fn update(&mut self, s: CounterSample, now: Instant) -> (f64, u64) {
        let mut live = s.finished.saturating_add(s.in_flight.unwrap_or(0));
        let rate = match self.prev {
            Some(p) if s.finished >= p.finished => {
                // Racing /metrics vs /slots can briefly under-count; clamp so
                // the total stays monotonic.
                live = live.max(p.live);
                let delta = (live - p.live) as f64;
                let wall = now.saturating_duration_since(p.at).as_secs_f64();
                // Without per-slot progress the counter only jumps at request
                // end, so divide by actual generation time rather than the
                // (much shorter) poll interval.
                let secs = if s.in_flight.is_none() {
                    wall.max(s.gen_seconds - p.gen_seconds)
                } else {
                    wall
                };
                if delta > 0.0 && secs > 0.0 {
                    delta / secs
                } else {
                    0.0
                }
            }
            _ => 0.0,
        };
        self.prev = Some(Prev {
            finished: s.finished,
            gen_seconds: s.gen_seconds,
            live,
            at: now,
        });
        (rate, live)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
# HELP llamacpp:prompt_tokens_total Number of prompt tokens processed.
# TYPE llamacpp:prompt_tokens_total counter
llamacpp:prompt_tokens_total 120
# HELP llamacpp:tokens_predicted_total Number of generation tokens processed.
# TYPE llamacpp:tokens_predicted_total counter
llamacpp:tokens_predicted_total 345
llamacpp:prompt_tokens_seconds 512.3
llamacpp:predicted_tokens_seconds 41.7
llamacpp:requests_processing 1
llamacpp:requests_deferred 0
";

    #[test]
    fn parses_sample_metrics() {
        let s = parse_prometheus(SAMPLE);
        assert_eq!(s.len(), 6);
        assert_eq!(
            metric_value(SAMPLE, "llamacpp:tokens_predicted_total"),
            Some(345.0)
        );
        assert_eq!(
            metric_value(SAMPLE, "llamacpp:prompt_tokens_total"),
            Some(120.0)
        );
        assert_eq!(
            metric_value(SAMPLE, "llamacpp:predicted_tokens_seconds"),
            Some(41.7)
        );
        assert_eq!(
            metric_value(SAMPLE, "llamacpp:requests_processing"),
            Some(1.0)
        );
        assert_eq!(metric_value(SAMPLE, "llamacpp:missing"), None);
    }

    #[test]
    fn parses_labels_timestamps_and_specials() {
        let text = "\
foo{model=\"a}b\",x=\"q\\\"z\"} 3.5 1700000000000
bar{} 1e3
baz +Inf
  indented 7
broken{unterminated 1
novalue
nan_metric NaN
";
        let s = parse_prometheus(text);
        assert_eq!(s[0].name, "foo");
        assert_eq!(s[0].labels, "model=\"a}b\",x=\"q\\\"z\"");
        assert_eq!(s[0].value, 3.5);
        assert_eq!(s[1].name, "bar");
        assert_eq!(s[1].value, 1000.0);
        assert!(s[2].value.is_infinite());
        assert_eq!(s[3].name, "indented");
        assert!(s[4].value.is_nan());
        assert_eq!(s.len(), 5);
    }

    #[test]
    fn empty_and_garbage_input() {
        assert!(parse_prometheus("").is_empty());
        assert!(parse_prometheus("<html>404 Not Found</html>").is_empty());
    }

    fn slots_sample(finished: u64, in_flight: u64) -> CounterSample {
        CounterSample {
            finished,
            gen_seconds: 0.0,
            in_flight: Some(in_flight),
        }
    }

    #[test]
    fn rate_first_sample_is_zero_then_delta() {
        let t0 = Instant::now();
        let mut r = RateTracker::default();
        assert_eq!(r.update(slots_sample(100, 0), t0), (0.0, 100));
        let (rate, total) = r.update(slots_sample(150, 0), t0 + Duration::from_secs(2));
        assert!((rate - 25.0).abs() < 1e-9);
        assert_eq!(total, 150);
    }

    #[test]
    fn rate_zero_when_counter_unchanged() {
        let t0 = Instant::now();
        let mut r = RateTracker::default();
        r.update(slots_sample(100, 0), t0);
        assert_eq!(
            r.update(slots_sample(100, 0), t0 + Duration::from_secs(1)),
            (0.0, 100)
        );
    }

    #[test]
    fn last_total_survives_missed_sample() {
        let mut r = RateTracker::default();
        assert_eq!(r.last_total(), None);
        let t0 = Instant::now();
        r.update(slots_sample(500, 0), t0);
        assert_eq!(r.last_total(), Some(500));
    }

    #[test]
    fn rate_handles_counter_reset() {
        let t0 = Instant::now();
        let mut r = RateTracker::default();
        r.update(slots_sample(10_000, 0), t0);
        // Server restarted: counter dropped. No negative/huge rate.
        assert_eq!(
            r.update(slots_sample(5, 0), t0 + Duration::from_secs(1)),
            (0.0, 5)
        );
        let (rate, total) = r.update(slots_sample(15, 0), t0 + Duration::from_secs(2));
        assert!((rate - 10.0).abs() < 1e-9);
        assert_eq!(total, 15);
    }

    #[test]
    fn rate_zero_elapsed_is_zero() {
        let t0 = Instant::now();
        let mut r = RateTracker::default();
        r.update(slots_sample(1, 0), t0);
        assert_eq!(r.update(slots_sample(50, 0), t0).0, 0.0);
    }

    /// Mirrors the live trace: counter 0 while generating (slot n_decoded
    /// climbs ~3 tok/s), then counter jumps to 40 and the slot frees.
    #[test]
    fn rate_live_via_slots_and_smooth_handoff() {
        let t0 = Instant::now();
        let at = |s| t0 + Duration::from_secs(s);
        let mut r = RateTracker::default();
        r.update(slots_sample(0, 0), at(0));
        let (rate, _) = r.update(slots_sample(0, 6), at(2));
        assert!((rate - 3.0).abs() < 1e-9);
        let (rate, _) = r.update(slots_sample(0, 37), at(12));
        assert!((rate - 3.1).abs() < 1e-9);
        // Request finished: no double count, no spike.
        let (rate, total) = r.update(slots_sample(40, 0), at(13));
        assert!((rate - 3.0).abs() < 1e-9);
        assert_eq!(total, 40);
    }

    #[test]
    fn rate_clamps_metrics_slots_race() {
        let t0 = Instant::now();
        let mut r = RateTracker::default();
        r.update(slots_sample(0, 37), t0);
        // Request finished after /metrics was read but before /slots was.
        assert_eq!(
            r.update(slots_sample(0, 0), t0 + Duration::from_secs(1)),
            (0.0, 37)
        );
        let (rate, total) = r.update(slots_sample(40, 0), t0 + Duration::from_secs(2));
        assert!((rate - 3.0).abs() < 1e-9);
        assert_eq!(total, 40);
    }

    #[test]
    fn rate_without_slots_uses_generation_seconds() {
        let t0 = Instant::now();
        let mut r = RateTracker::default();
        let s = |finished, gen_seconds| CounterSample {
            finished,
            gen_seconds,
            in_flight: None,
        };
        r.update(s(0, 0.0), t0);
        assert_eq!(r.update(s(0, 0.0), t0 + Duration::from_secs(1)).0, 0.0);
        // 40 tokens over 13.66s of generation, observed within a 1s poll.
        let (rate, total) = r.update(s(40, 13.6609), t0 + Duration::from_secs(2));
        assert!((rate - 40.0 / 13.6609).abs() < 1e-6);
        assert_eq!(total, 40);
    }

    #[test]
    fn counters_and_slots_parsing() {
        assert_eq!(counters_from_metrics(SAMPLE), Some((345, 0.0)));
        let text =
            "llamacpp:tokens_predicted_total 40\nllamacpp:tokens_predicted_seconds_total 13.6609\n";
        assert_eq!(counters_from_metrics(text), Some((40, 13.6609)));
        assert_eq!(counters_from_metrics("other 1"), None);

        let slots: Vec<Slot> = serde_json::from_str(
            r#"[
              {"id":0,"is_processing":false,"next_token":[{"n_decoded":99}]},
              {"id":1,"is_processing":true,"next_token":[{"has_next_token":true,"n_decoded":14}]},
              {"id":2,"is_processing":true,"next_token":{"n_decoded":5}},
              {"id":3,"is_processing":true}
            ]"#,
        )
        .unwrap();
        assert_eq!(in_flight_tokens(&slots), 19);
    }

    #[test]
    fn models_list_and_props_fallback() {
        let r: ModelsResponse = serde_json::from_str(
            r#"{"models":[{"name":"x"}],"object":"list","data":[
                {"id":"/mnt/arc1/models/gemma-4-31B-it-Q4_K_M.gguf","object":"model"},
                {"id":"unsloth/qwen3"},{"id":""}]}"#,
        )
        .unwrap();
        assert_eq!(
            models_from_response(r),
            vec!["gemma-4-31B-it-Q4_K_M", "unsloth/qwen3"]
        );

        let p: PropsResponse =
            serde_json::from_str(r#"{"model_path":"/mnt/models/gemma.GGUF"}"#).unwrap();
        assert_eq!(name_from_props(&p).as_deref(), Some("gemma"));
        let p: PropsResponse =
            serde_json::from_str(r#"{"model_alias":"g","model_path":"/x/y.gguf"}"#).unwrap();
        assert_eq!(name_from_props(&p).as_deref(), Some("g"));
        let p: PropsResponse = serde_json::from_str(r#"{"model_alias":"  "}"#).unwrap();
        assert_eq!(name_from_props(&p), None);
    }

    #[tokio::test]
    async fn unreachable_returns_empty() {
        let mut c = LlamaCpp::new("http://127.0.0.1:1".into());
        assert!(c.poll().await.is_empty());
    }

    /// Live check: `LLMTOP_LLAMACPP_URL=http://127.0.0.1:18080 cargo test -- --ignored live`
    #[tokio::test]
    #[ignore]
    async fn live_llama_server() {
        let url = std::env::var("LLMTOP_LLAMACPP_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:18080".into());
        let mut c = LlamaCpp::new(url);
        for _ in 0..5 {
            let models = c.poll().await;
            eprintln!("{models:?}");
            assert!(!models.is_empty());
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
}
