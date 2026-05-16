use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    symbols::Marker,
    text::Span,
    widgets::{Axis, Block, Borders, Chart, Dataset, GraphType},
};

const ACCENT: Color = Color::Green;
const DANGER: Color = Color::Red;

use crate::app::App;

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    draw_util(f, cols[0], app);
    draw_power(f, cols[1], app);
}

fn draw_util(f: &mut Frame, area: Rect, app: &App) {
    let cur = app.hardware.as_ref().map(|h| h.gpu_util_pct).unwrap_or(0);
    let color = accent_for(cur as f64, 100.0);

    let points = smoothed(&app.util_history, 0.35);

    let title_spans = vec![Span::raw(format!(" GPU util {:>3}% ", cur))];

    let datasets = vec![
        Dataset::default()
            .marker(Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(color))
            .data(&points),
    ];

    let xmax = points.last().map(|(x, _)| *x).unwrap_or(0.0).max(1.0);
    let xmin = (xmax - 120.0).max(0.0);

    let chart = Chart::new(datasets)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(title_spans),
        )
        .x_axis(Axis::default().bounds([xmin, xmax]))
        .y_axis(
            Axis::default()
                .style(Style::default().fg(Color::DarkGray))
                .bounds([0.0, 100.0])
                .labels(vec![
                    Span::styled("0", Style::default().fg(Color::DarkGray)),
                    Span::styled("50", Style::default().fg(Color::DarkGray)),
                    Span::styled("100", Style::default().fg(Color::DarkGray)),
                ]),
        );
    f.render_widget(chart, area);
}

fn draw_power(f: &mut Frame, area: Rect, app: &App) {
    let cur = app.hardware.as_ref().map(|h| h.power_w).unwrap_or(0.0);
    let limit = app
        .hardware
        .as_ref()
        .map(|h| h.power_limit_w.max(1.0))
        .unwrap_or(500.0);

    let points = smoothed(&app.power_history, 0.35);

    let color = accent_for(cur, limit);
    let title_spans = vec![Span::raw(format!(" Power {:>3.0}W / {:.0}W ", cur, limit))];

    let datasets = vec![
        Dataset::default()
            .marker(Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(color))
            .data(&points),
    ];

    let xmax = points.last().map(|(x, _)| *x).unwrap_or(0.0).max(1.0);
    let xmin = (xmax - 120.0).max(0.0);
    let half = limit / 2.0;

    let chart = Chart::new(datasets)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(title_spans),
        )
        .x_axis(Axis::default().bounds([xmin, xmax]))
        .y_axis(
            Axis::default()
                .style(Style::default().fg(Color::DarkGray))
                .bounds([0.0, limit])
                .labels(vec![
                    Span::styled("0", Style::default().fg(Color::DarkGray)),
                    Span::styled(format!("{:.0}", half), Style::default().fg(Color::DarkGray)),
                    Span::styled(format!("{:.0}", limit), Style::default().fg(Color::DarkGray)),
                ]),
        );
    f.render_widget(chart, area);
}

fn smoothed(hist: &[f64], alpha: f64) -> Vec<(f64, f64)> {
    let mut out = Vec::with_capacity(hist.len());
    let mut ema = 0.0;
    for (i, v) in hist.iter().enumerate() {
        ema = if i == 0 { *v } else { alpha * v + (1.0 - alpha) * ema };
        out.push((i as f64, ema));
    }
    out
}

fn accent_for(v: f64, max: f64) -> Color {
    let r = if max > 0.0 { v / max } else { 0.0 };
    if r >= 0.9 { DANGER } else { ACCENT }
}
