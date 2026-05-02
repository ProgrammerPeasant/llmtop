use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Sparkline},
};

use crate::app::App;

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let cols = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Length(4)])
        .split(area);

    let util_data: Vec<u64> = app.util_history.iter().map(|v| *v as u64).collect();
    let util_title = match app.hardware.as_ref() {
        Some(h) => format!(" GPU util · {}% ", h.gpu_util_pct),
        None => " GPU util ".into(),
    };
    let util = Sparkline::default()
        .block(Block::default().borders(Borders::ALL).title(util_title))
        .data(&util_data)
        .max(100)
        .style(Style::default().fg(Color::Cyan));
    f.render_widget(util, cols[0]);

    let power_data: Vec<u64> = app.power_history.iter().map(|v| *v as u64).collect();
    let power_title = match app.hardware.as_ref() {
        Some(h) => format!(" Power · {:.0} W / {:.0} W ", h.power_w, h.power_limit_w),
        None => " Power ".into(),
    };
    let max = app
        .hardware
        .as_ref()
        .map(|h| h.power_limit_w.max(1.0) as u64)
        .unwrap_or(500);
    let power = Sparkline::default()
        .block(Block::default().borders(Borders::ALL).title(power_title))
        .data(&power_data)
        .max(max)
        .style(Style::default().fg(Color::Yellow));
    f.render_widget(power, cols[1]);
}
