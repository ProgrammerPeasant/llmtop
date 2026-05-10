use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols::Marker,
    text::Span,
    widgets::{Axis, Block, Borders, Chart, Dataset, GraphType},
};

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
    let color = util_color(cur);

    let points: Vec<(f64, f64)> = app
        .util_history
        .iter()
        .enumerate()
        .map(|(i, v)| (i as f64, *v))
        .collect();

    let title_color = title_color_for(cur as f64, 100.0);
    let title_spans = vec![
        Span::styled(" GPU util ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        Span::styled(
            format!("{:>3}%", cur),
            Style::default().fg(title_color).add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
    ];

    let datasets = vec![
        Dataset::default()
            .marker(Marker::HalfBlock)
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
                    Span::styled("100%", Style::default().fg(Color::DarkGray)),
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

    let points: Vec<(f64, f64)> = app
        .power_history
        .iter()
        .enumerate()
        .map(|(i, v)| (i as f64, *v))
        .collect();

    let title_color = title_color_for(cur, limit);
    let title_spans = vec![
        Span::styled(" Power ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        Span::styled(
            format!("{:>3.0} W", cur),
            Style::default().fg(title_color).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" / {:.0} W ", limit), Style::default().fg(Color::DarkGray)),
    ];

    let datasets = vec![
        Dataset::default()
            .marker(Marker::HalfBlock)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(Color::Yellow))
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
                    Span::styled(format!("{:.0}W", limit), Style::default().fg(Color::DarkGray)),
                ]),
        );
    f.render_widget(chart, area);
}

fn util_color(pct: u32) -> Color {
    match pct {
        0..=20 => Color::Green,
        21..=70 => Color::Cyan,
        71..=90 => Color::LightYellow,
        _ => Color::LightRed,
    }
}

fn title_color_for(v: f64, max: f64) -> Color {
    let r = if max > 0.0 { v / max } else { 0.0 };
    if r < 0.2 {
        Color::Green
    } else if r < 0.7 {
        Color::Cyan
    } else if r < 0.9 {
        Color::LightYellow
    } else {
        Color::LightRed
    }
}
