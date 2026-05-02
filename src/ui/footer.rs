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
    let co2_kg = wh_to_g_co2(app.session_wh, app.cli.grid_co2) / 1000.0;

    let pause_marker = if app.paused { " [PAUSED]" } else { "" };
    let txt = format!(
        "Session {h}:{m:02}:{s:02} · {kwh:.3} kWh · ${cost:.2} · {co2_kg:.3} kg CO₂eq · tokens: {tok}{p}\n[q]uit  [p]ause  [c]lear",
        tok = app.session_tokens,
        p = pause_marker,
    );

    let para = Paragraph::new(txt)
        .style(Style::default().fg(Color::Gray))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(para, area);
}
