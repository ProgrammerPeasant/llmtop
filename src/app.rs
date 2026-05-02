use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind};
use std::time::Instant;

use crate::{
    collectors::{HardwareSnapshot, ModelInfo},
    config::Cli,
};

pub struct App {
    pub cli: Cli,
    pub started: Instant,
    pub hardware: Option<HardwareSnapshot>,
    pub models: Vec<ModelInfo>,
    pub power_history: Vec<f64>,
    pub util_history: Vec<f64>,
    pub session_wh: f64,
    pub session_tokens: u64,
    pub paused: bool,
    last_sample: Option<Instant>,
}

impl App {
    pub fn new(cli: Cli) -> Self {
        Self {
            cli,
            started: Instant::now(),
            hardware: None,
            models: Vec::new(),
            power_history: Vec::with_capacity(240),
            util_history: Vec::with_capacity(240),
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
                KeyCode::Char('q') | KeyCode::Esc => return true,
                KeyCode::Char('p') => self.paused = !self.paused,
                KeyCode::Char('c') => self.reset_session(),
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
        self.hardware = Some(snap);
    }

    pub fn update_ollama(&mut self, models: Vec<ModelInfo>) {
        if self.paused {
            return;
        }
        // Accumulate tokens delta per model name.
        let mut delta: u64 = 0;
        for new in &models {
            if let Some(prev) = self.models.iter().find(|m| m.name == new.name)
                && new.total_tokens >= prev.total_tokens
            {
                delta += new.total_tokens - prev.total_tokens;
            }
        }
        self.session_tokens += delta;
        self.models = models;
    }
}

fn push_capped(buf: &mut Vec<f64>, v: f64, cap: usize) {
    if buf.len() >= cap {
        buf.remove(0);
    }
    buf.push(v);
}
