use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use std::time::Instant;

use crate::{
    collectors::{HardwareSnapshot, ModelInfo},
    config::Cli,
};

/// Which backend a model row came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Ollama,
    LlamaCpp,
}

impl Source {
    pub fn label(self) -> &'static str {
        match self {
            Source::Ollama => "ollama",
            Source::LlamaCpp => "llama.cpp",
        }
    }

    /// Whether this backend reports per-model VRAM.
    pub fn reports_vram(self) -> bool {
        matches!(self, Source::Ollama)
    }
}

/// One row of the combined model view.
#[derive(Debug, Clone)]
pub struct ModelRow {
    pub source: Source,
    pub info: ModelInfo,
}

pub struct App {
    pub cli: Cli,
    pub started: Instant,
    pub hardware: Option<HardwareSnapshot>,
    /// Latest snapshot per source; also the baseline for token deltas.
    ollama_models: Vec<ModelInfo>,
    llamacpp_models: Vec<ModelInfo>,
    /// Combined view used by the UI (ollama first, then llama.cpp).
    /// Rebuilt on every source update.
    pub models: Vec<ModelRow>,
    pub power_history: Vec<f64>,
    pub util_history: Vec<f64>,
    /// Combined tok/s across all models, sampled on each hardware tick.
    pub tok_s_history: Vec<f64>,
    /// GPU VRAM used (MB, summed across GPUs), sampled on each hardware tick.
    pub vram_history: Vec<f64>,
    /// `g` toggles a second chart row (tok/s + VRAM).
    pub show_llm_charts: bool,
    /// `?` shows the key list over everything else.
    pub show_help: bool,
    pub theme: crate::ui::theme::Theme,
    pub session_wh: f64,
    pub session_tokens: u64,
    pub paused: bool,
    last_sample: Option<Instant>,
}

impl App {
    pub fn new(cli: Cli) -> Self {
        let theme = crate::ui::theme::Theme::resolve(cli.no_color);
        Self {
            cli,
            started: Instant::now(),
            hardware: None,
            ollama_models: Vec::new(),
            llamacpp_models: Vec::new(),
            models: Vec::new(),
            power_history: Vec::with_capacity(240),
            util_history: Vec::with_capacity(240),
            tok_s_history: Vec::with_capacity(240),
            vram_history: Vec::with_capacity(240),
            show_llm_charts: false,
            show_help: false,
            theme,
            session_wh: 0.0,
            session_tokens: 0,
            paused: false,
            last_sample: None,
        }
    }

    /// Returns true if app should quit.
    pub fn handle_input(&mut self, ev: Event) -> bool {
        if let Event::Key(KeyEvent {
            code,
            kind: KeyEventKind::Press,
            ..
        }) = ev
        {
            match code {
                // Esc / ? close the help screen first; q always quits.
                KeyCode::Esc | KeyCode::Char('?') if self.show_help => self.show_help = false,
                KeyCode::Char('?') => self.show_help = true,
                KeyCode::Char('q') | KeyCode::Esc => return true,
                KeyCode::Char('p') => self.paused = !self.paused,
                KeyCode::Char('c') => self.reset_session(),
                KeyCode::Char('g') => self.show_llm_charts = !self.show_llm_charts,
                _ => {}
            }
        }
        false
    }

    fn reset_session(&mut self) {
        self.started = Instant::now();
        self.session_wh = 0.0;
        self.session_tokens = 0;
        self.power_history.clear();
        self.util_history.clear();
        self.tok_s_history.clear();
        self.vram_history.clear();
        self.last_sample = None;
    }

    pub fn update_hardware(&mut self, snap: HardwareSnapshot) {
        if self.paused {
            return;
        }
        // Integrate energy: W * dt(h) = Wh.
        let now = Instant::now();
        if let Some(prev) = self.last_sample {
            let dt_h = now.duration_since(prev).as_secs_f64() / 3600.0;
            self.session_wh += snap.power_w * dt_h;
        }
        self.last_sample = Some(now);

        push_capped(&mut self.power_history, snap.power_w, 240);
        push_capped(&mut self.util_history, snap.gpu_util_pct as f64, 240);
        let tok_s = self.total_tok_s();
        push_capped(&mut self.tok_s_history, tok_s, 240);
        push_capped(&mut self.vram_history, snap.vram_used_mb as f64, 240);
        self.hardware = Some(snap);
    }

    /// Sum of live tok/s across every model from every backend.
    pub fn total_tok_s(&self) -> f64 {
        self.models.iter().map(|r| r.info.tokens_per_sec).sum()
    }

    pub fn update_ollama(&mut self, models: Vec<ModelInfo>) {
        self.update_source(Source::Ollama, models);
    }

    pub fn update_llamacpp(&mut self, models: Vec<ModelInfo>) {
        self.update_source(Source::LlamaCpp, models);
    }

    fn update_source(&mut self, source: Source, models: Vec<ModelInfo>) {
        if self.paused {
            return;
        }
        let prev = match source {
            Source::Ollama => &mut self.ollama_models,
            Source::LlamaCpp => &mut self.llamacpp_models,
        };
        self.session_tokens += token_delta(prev, &models);
        *prev = models;
        self.rebuild_models();
    }

    fn rebuild_models(&mut self) {
        let tag = |source: Source, v: &[ModelInfo]| {
            v.iter()
                .cloned()
                .map(move |info| ModelRow { source, info })
                .collect::<Vec<_>>()
        };
        let mut combined = tag(Source::Ollama, &self.ollama_models);
        combined.extend(tag(Source::LlamaCpp, &self.llamacpp_models));
        self.models = combined;
    }
}

/// Sum of per-model `total_tokens` increases between two snapshots of the
/// same source, matched by name. New models (no baseline) and counter resets
/// (new < prev, e.g. server restart) contribute nothing.
fn token_delta(prev: &[ModelInfo], new: &[ModelInfo]) -> u64 {
    new.iter()
        .filter_map(|n| {
            prev.iter()
                .find(|p| p.name == n.name)
                .map(|p| n.total_tokens.saturating_sub(p.total_tokens))
        })
        .sum()
}

fn push_capped(buf: &mut Vec<f64>, v: f64, cap: usize) {
    if buf.len() >= cap {
        buf.remove(0);
    }
    buf.push(v);
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    fn m(name: &str, total: u64) -> ModelInfo {
        ModelInfo {
            name: name.into(),
            vram_mb: 0,
            tokens_per_sec: 0.0,
            total_tokens: total,
            ctx_size: None,
        }
    }

    fn app() -> App {
        App::new(Cli::parse_from(["llmtop"]))
    }

    #[test]
    fn delta_basic_and_reset() {
        assert_eq!(token_delta(&[], &[m("a", 100)]), 0);
        assert_eq!(token_delta(&[m("a", 100)], &[m("a", 150)]), 50);
        // Counter reset: no negative / wrapped contribution.
        assert_eq!(token_delta(&[m("a", 150)], &[m("a", 10)]), 0);
        // Only matched names count.
        assert_eq!(
            token_delta(&[m("a", 1), m("b", 5)], &[m("a", 3), m("c", 99)]),
            2
        );
    }

    #[test]
    fn sources_do_not_clobber_each_other() {
        let mut a = app();
        a.update_ollama(vec![m("qwen", 100)]);
        a.update_llamacpp(vec![m("qwen", 1000)]);
        assert_eq!(a.session_tokens, 0);
        assert_eq!(a.models.len(), 2);

        // Interleaved updates: each source diffs against its own baseline.
        a.update_ollama(vec![m("qwen", 110)]);
        assert_eq!(a.session_tokens, 10);
        a.update_llamacpp(vec![m("qwen", 1050)]);
        assert_eq!(a.session_tokens, 60);
        a.update_ollama(vec![m("qwen", 115)]);
        assert_eq!(a.session_tokens, 65);

        assert_eq!(a.models[0].source, Source::Ollama);
        assert_eq!(a.models[0].info.total_tokens, 115);
        assert_eq!(a.models[1].source, Source::LlamaCpp);
        assert_eq!(a.models[1].info.total_tokens, 1050);
    }

    #[test]
    fn counter_reset_in_one_source_only() {
        let mut a = app();
        a.update_ollama(vec![m("x", 100)]);
        a.update_llamacpp(vec![m("y", 500)]);
        // llama-server restarted: counter drops.
        a.update_llamacpp(vec![m("y", 20)]);
        assert_eq!(a.session_tokens, 0);
        // Counting resumes from the new baseline.
        a.update_llamacpp(vec![m("y", 30)]);
        a.update_ollama(vec![m("x", 105)]);
        assert_eq!(a.session_tokens, 15);
    }

    #[test]
    fn empty_source_keeps_other() {
        let mut a = app();
        a.update_ollama(vec![m("x", 1)]);
        a.update_llamacpp(vec![]);
        assert_eq!(a.models.len(), 1);
        a.update_ollama(vec![]);
        assert!(a.models.is_empty());
    }

    #[test]
    fn pause_freezes_and_reset_clears() {
        let mut a = app();
        a.update_llamacpp(vec![m("y", 10)]);
        a.paused = true;
        a.update_llamacpp(vec![m("y", 50)]);
        assert_eq!(a.session_tokens, 0);
        assert_eq!(a.models[0].info.total_tokens, 10);
        a.paused = false;
        // Tokens generated while paused are counted on resume (same as
        // previous single-source behaviour, since the baseline was frozen).
        a.update_llamacpp(vec![m("y", 60)]);
        assert_eq!(a.session_tokens, 50);

        a.reset_session();
        assert_eq!(a.session_tokens, 0);
        a.update_llamacpp(vec![m("y", 65)]);
        assert_eq!(a.session_tokens, 5);
    }
}
