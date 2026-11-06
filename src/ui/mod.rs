//! Screen layout in the tui-tools style (tui-kit): a flat header with
//! `label: value` facts, a borderless table with a magenta header row, the
//! charts in rounded muted panels, then the session facts, the key hint bar
//! and a muted status line. `?` opens the key list as an accent dialog.

mod footer;
mod sparkline;
mod table;
pub mod theme;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Flex, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Padding, Paragraph},
};

use crate::app::{App, Source};
use theme::Theme;

pub fn draw(f: &mut Frame, app: &App) {
    // One cell of horizontal padding everywhere, like tui-kit's Padding(0, 1).
    let area = Block::default()
        .padding(Padding::horizontal(1))
        .inner(f.area());

    let header = header_lines(app, area.width);
    let chart_rows: u16 = if app.show_llm_charts { 2 } else { 1 };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(header.len() as u16 + 1), // header + gap
            Constraint::Min(3),                          // models table
            Constraint::Length(11 * chart_rows),         // charts: GPU row, + tok/s/VRAM on `g`
            Constraint::Length(3),                       // session facts, keys, status
        ])
        .split(area);

    f.render_widget(Paragraph::new(header), chunks[0]);
    table::draw(f, chunks[1], app);
    sparkline::draw(f, chunks[2], app);
    footer::draw(f, chunks[3], app);

    if app.show_help {
        help_screen(f, app);
    }
}

/// Title line plus the hardware facts, wrapped to the width.
fn header_lines(app: &App, width: u16) -> Vec<Line<'static>> {
    let t = &app.theme;
    let mut lines = vec![Line::from(vec![
        Span::styled("llmtop", t.title()),
        Span::styled("  realtime monitor for local LLMs", t.muted()),
    ])];

    let mut facts: Vec<(&str, String, ratatui::style::Style)> = Vec::new();
    match app.hardware.as_ref().filter(|h| !h.gpu_name.is_empty()) {
        Some(h) => {
            facts.push(("gpu", h.gpu_name.clone(), t.base()));
            facts.push(("util", format!("{}%", h.gpu_util_pct), t.base()));
            if h.vram_total_mb > 0 {
                facts.push((
                    "vram",
                    format!(
                        "{:.1}G / {:.0}G",
                        h.vram_used_mb as f64 / 1024.0,
                        h.vram_total_mb as f64 / 1024.0
                    ),
                    t.base(),
                ));
            }
            facts.push((
                "power",
                format!("{:.0}W / {:.0}W", h.power_w, h.power_limit_w),
                t.base(),
            ));
            facts.push(("temp", format!("{}°C", h.temp_c), t.base()));
        }
        None => facts.push(("gpu", "not detected".into(), t.muted())),
    }
    let backends = backends(app);
    facts.push((
        "backends",
        if backends.is_empty() {
            "none".into()
        } else {
            backends.join(", ")
        },
        t.base(),
    ));
    facts.push(("compare", app.cli.compare.clone(), t.base()));

    lines.extend(fact_lines(t, &facts, width));
    lines
}

/// Lay `label: value` facts out on as many lines as the width needs
/// (tui-kit's `renderFacts`).
pub(crate) fn fact_lines(
    t: &Theme,
    facts: &[(&str, String, ratatui::style::Style)],
    width: u16,
) -> Vec<Line<'static>> {
    const SEP: &str = "   ";
    let mut lines = Vec::new();
    let mut cur: Vec<Span<'static>> = Vec::new();
    let mut used = 0usize;
    for (label, value, style) in facts {
        let cost = label.chars().count() + 2 + value.chars().count();
        if !cur.is_empty() && used + SEP.len() + cost > width as usize {
            lines.push(Line::from(std::mem::take(&mut cur)));
            used = 0;
        }
        if !cur.is_empty() {
            cur.push(Span::raw(SEP));
            used += SEP.len();
        }
        cur.push(Span::styled(format!("{label}: "), t.muted()));
        cur.push(Span::styled(value.clone(), *style));
        used += cost;
    }
    if !cur.is_empty() {
        lines.push(Line::from(cur));
    }
    lines
}

/// Backends being polled, by label.
pub(crate) fn backends(app: &App) -> Vec<&'static str> {
    let mut v = Vec::new();
    if app.cli.ollama_enabled() {
        v.push(Source::Ollama.label());
    }
    if app.cli.llamacpp_url.is_some() {
        v.push(Source::LlamaCpp.label());
    }
    v
}

/// Key list for the hint bar (short) and help screen (long).
pub(crate) const KEYS: &[(&str, &str, &str)] = &[
    ("p", "pause", "pause and resume sampling"),
    ("c", "clear", "clear the session totals and charts"),
    ("g", "charts", "show / hide the tok/s and VRAM charts"),
    ("?", "help", "this help"),
    ("q", "quit", "quit (esc too, when no help is open)"),
];

/// The key list as a bordered accent dialog, centered (tui-kit `HelpScreen`).
fn help_screen(f: &mut Frame, app: &App) {
    let t = &app.theme;
    let key_w = KEYS.iter().map(|(k, ..)| k.len()).max().unwrap_or(1).max(4);
    let row = |k: &str, d: &str| {
        Line::from(vec![
            Span::styled(format!("{k:<key_w$}"), t.key()),
            Span::raw("  "),
            Span::styled(d.to_string(), t.muted()),
        ])
    };

    let mut lines = vec![
        Line::from(Span::styled("llmtop — keys", t.title())),
        Line::raw(""),
    ];
    lines.extend(KEYS.iter().map(|(k, _, long)| row(k, long)));
    lines.push(Line::raw(""));
    lines.push(row(
        "note",
        "llmtop only reads: nothing here changes your servers",
    ));

    let content_w = lines.iter().map(Line::width).max().unwrap_or(0) as u16;
    let w = (content_w + 6).min(f.area().width);
    let h = (lines.len() as u16 + 4).min(f.area().height);
    let area = centered(f.area(), w, h);

    let dialog = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(t.dialog_border())
        .padding(Padding::new(2, 2, 1, 1));
    f.render_widget(Clear, area);
    f.render_widget(Paragraph::new(lines).block(dialog), area);
}

fn centered(area: Rect, w: u16, h: u16) -> Rect {
    let [v] = Layout::vertical([Constraint::Length(h)])
        .flex(Flex::Center)
        .areas(area);
    let [r] = Layout::horizontal([Constraint::Length(w)])
        .flex(Flex::Center)
        .areas(v);
    r
}
