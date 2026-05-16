use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
};

use crate::{app::App, pricing::co2::wh_to_g_co2};

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let elapsed = app.started.elapsed();
    let h = elapsed.as_secs() / 3600;
    let m = (elapsed.as_secs() % 3600) / 60;
    let s = elapsed.as_secs() % 60;

    let kwh = app.session_wh / 1000.0;
    let usd_kwh = 0.15; // rough world residential avg, override later via CLI
    let cost = kwh * usd_kwh;
    let g_co2 = wh_to_g_co2(app.session_wh, app.cli.grid_co2);

    let pause_marker = if app.paused { " [PAUSED]" } else { "" };
    let txt = format!(
        "Session {h}:{m:02}:{s:02}  {energy}  {cost_str}  {co2} CO2  tokens: {tok}{p}\nq:quit  p:pause  c:clear",
        energy = fmt_energy(app.session_wh),
        cost_str = fmt_cost(cost),
        co2 = fmt_co2(g_co2),
        tok = app.session_tokens,
        p = pause_marker,
    );
    let _ = kwh; // kept for future CLI exposure of $/kWh.

    let para = Paragraph::new(txt)
        .style(Style::default().fg(Color::Gray))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(para, area);
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
    if !(usd > 0.0) {
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
