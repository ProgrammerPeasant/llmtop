use ratatui::{
    Frame,
    layout::Rect,
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::{app::App, pricing::co2::wh_to_g_co2};

/// Three lines: session facts, key hint bar, status line.
pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;
    let elapsed = app.started.elapsed().as_secs();
    let (h, m, s) = (elapsed / 3600, (elapsed % 3600) / 60, elapsed % 60);

    let usd_kwh = 0.15; // rough world residential avg, override later via CLI
    let cost = app.session_wh / 1000.0 * usd_kwh;
    let g_co2 = wh_to_g_co2(app.session_wh, app.cli.grid_co2);

    let facts = [
        ("session", format!("{h}:{m:02}:{s:02}"), t.base()),
        ("energy", fmt_energy(app.session_wh), t.base()),
        ("cost", fmt_cost(cost), t.base()),
        ("co2", fmt_co2(g_co2), t.base()),
        ("tokens", app.session_tokens.to_string(), t.base()),
    ];
    // The facts stay on one line here; the hint bar and status line follow.
    let facts_line = super::fact_lines(t, &facts, u16::MAX).remove(0);

    let lines = vec![facts_line, key_bar(app, area.width), status_line(app)];
    f.render_widget(Paragraph::new(lines), area);
}

/// `key desc` pairs, dropping the ones that don't fit (tui-kit `HelpBar`).
fn key_bar(app: &App, width: u16) -> Line<'static> {
    let t = &app.theme;
    let mut spans = Vec::new();
    let mut used = 0usize;
    for (key, short, _) in super::KEYS {
        let desc = match *key {
            "p" if app.paused => "resume",
            _ => short,
        };
        let cost = key.len() + 1 + desc.len() + if spans.is_empty() { 0 } else { 2 };
        if used + cost > width as usize {
            break;
        }
        if !spans.is_empty() {
            spans.push(Span::raw("  "));
        }
        spans.push(Span::styled(key.to_string(), t.key()));
        spans.push(Span::styled(format!(" {desc}"), t.muted()));
        used += cost;
    }
    Line::from(spans)
}

/// A message when there is one (paused, theme warning), else a muted summary.
fn status_line(app: &App) -> Line<'static> {
    let t = &app.theme;
    if app.paused {
        return Line::from(Span::styled(
            "paused: sampling stopped, p to resume",
            t.warn(),
        ));
    }
    if let Some(w) = &app.theme.warning {
        return Line::from(Span::styled(w.clone(), t.warn()));
    }
    let n = app.models.len();
    let models = format!("{n} model{}", if n == 1 { "" } else { "s" });
    let polling = match super::backends(app).as_slice() {
        [] => "no backends enabled".to_string(),
        b => format!("polling {} every 1s", b.join(" + ")),
    };
    Line::from(Span::styled(
        format!("{models}  ·  {polling}  ·  ? for help"),
        t.muted(),
    ))
}

/// Wh until 1000, then kWh. Real sessions sit in single-Wh range; kWh
/// shows three leading zeros and reads as nothing happened.
fn fmt_energy(wh: f64) -> String {
    if wh < 1000.0 {
        format!("{wh:.1}Wh")
    } else {
        format!("{:.2}kWh", wh / 1000.0)
    }
}

/// g until 1000, then kg. A 4×A100 farm at full tilt is ~700 g/h —
/// kg only kicks in for hours-long sessions.
fn fmt_co2(g: f64) -> String {
    if g < 1000.0 {
        format!("{g:.1}g")
    } else {
        format!("{:.2}kg", g / 1000.0)
    }
}

/// Always dollars. For tiny values, expand decimals so two significant
/// digits remain visible — `$0.00` is useless feedback.
fn fmt_cost(usd: f64) -> String {
    if usd <= 0.0 || usd.is_nan() {
        return "$0.00".into();
    }
    if usd >= 1.0 {
        return format!("${usd:.2}");
    }
    // 0.05 → 2 decimals ($0.05); 0.009 → 4 ($0.0090); 0.00007 → 6 ($0.000070).
    let leading_zeros = (-usd.log10()).ceil().max(1.0) as usize;
    let decimals = (leading_zeros + 1).clamp(2, 8);
    format!("${usd:.*}", decimals)
}
