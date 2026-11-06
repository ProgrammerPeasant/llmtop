//! History charts in rounded, muted panels; titles in the accent color with
//! the current value in the foreground.

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    symbols::Marker,
    text::{Line, Span},
    widgets::{Axis, Block, BorderType, Borders, Chart, Dataset, GraphType},
};

use super::theme::Theme;
use crate::app::App;

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(if app.show_llm_charts {
            vec![Constraint::Percentage(50), Constraint::Percentage(50)]
        } else {
            vec![Constraint::Percentage(100)]
        })
        .split(area);

    let [util_area, power_area] = halves(rows[0]);
    draw_util(f, util_area, app);
    draw_power(f, power_area, app);

    if app.show_llm_charts {
        let [tok_area, vram_area] = halves(rows[1]);
        draw_tok_s(f, tok_area, app);
        draw_vram(f, vram_area, app);
    }
}

fn halves(area: Rect) -> [Rect; 2] {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);
    [cols[0], cols[1]]
}

fn draw_util(f: &mut Frame, area: Rect, app: &App) {
    let cur = app.hardware.as_ref().map(|h| h.gpu_util_pct).unwrap_or(0);
    let t = &app.theme;
    line_chart(
        f,
        area,
        t,
        Spec {
            title: ("GPU util", format!("{cur}%")),
            history: &app.util_history,
            ymax: 100.0,
            color: load_color(t, cur as f64, 100.0),
            label: &|v| format!("{v:.0}"),
        },
    );
}

fn draw_power(f: &mut Frame, area: Rect, app: &App) {
    let cur = app.hardware.as_ref().map(|h| h.power_w).unwrap_or(0.0);
    let limit = app
        .hardware
        .as_ref()
        .map(|h| h.power_limit_w.max(1.0))
        .unwrap_or(500.0);
    let t = &app.theme;
    line_chart(
        f,
        area,
        t,
        Spec {
            title: ("Power", format!("{cur:.0}W / {limit:.0}W")),
            history: &app.power_history,
            ymax: limit,
            color: load_color(t, cur, limit),
            label: &|v| format!("{v:.0}"),
        },
    );
}

fn draw_tok_s(f: &mut Frame, area: Rect, app: &App) {
    let cur = app.total_tok_s();
    let peak = app.tok_s_history.iter().copied().fold(0.0, f64::max);
    let t = &app.theme;
    line_chart(
        f,
        area,
        t,
        Spec {
            title: ("Tok/s", format!("{cur:.1}  peak {peak:.1}")),
            history: &app.tok_s_history,
            ymax: nice_ceil(peak),
            color: t.palette.cyan,
            label: &|v| format!("{v:.0}"),
        },
    );
}

fn draw_vram(f: &mut Frame, area: Rect, app: &App) {
    let (used, total) = app
        .hardware
        .as_ref()
        .map(|h| (h.vram_used_mb as f64, h.vram_total_mb as f64))
        .unwrap_or((0.0, 0.0));
    let ymax = if total > 0.0 { total } else { nice_ceil(used) };
    let value = if total > 0.0 {
        format!("{:.1}G / {:.0}G", used / 1024.0, total / 1024.0)
    } else {
        "no GPU data".to_string()
    };
    let t = &app.theme;
    let color = if used / ymax >= 0.9 {
        t.palette.red
    } else {
        t.palette.blue
    };
    line_chart(
        f,
        area,
        t,
        Spec {
            title: ("VRAM", value),
            history: &app.vram_history,
            ymax,
            color,
            label: &|v| format!("{:.0}G", v / 1024.0),
        },
    );
}

/// What one chart plots.
struct Spec<'a> {
    /// Accent label and current value for the panel title.
    title: (&'a str, String),
    history: &'a [f64],
    ymax: f64,
    color: Color,
    /// Y-axis label formatter.
    label: &'a dyn Fn(f64) -> String,
}

/// Braille line chart over the last 120 samples, y in `[0, ymax]` with
/// 0 / mid / max labels.
fn line_chart(f: &mut Frame, area: Rect, t: &Theme, spec: Spec<'_>) {
    let Spec {
        title: (label_text, value),
        history,
        ymax,
        color,
        label,
    } = spec;
    let points = smoothed(history, 0.35);
    let datasets = vec![
        Dataset::default()
            .marker(Marker::Braille)
            .graph_type(GraphType::Line)
            .style(t.series(color))
            .data(&points),
    ];

    let xmax = points.last().map(|(x, _)| *x).unwrap_or(0.0).max(1.0);
    let xmin = (xmax - 120.0).max(0.0);
    let ymax = ymax.max(1.0);
    let dim: Style = t.muted();
    let title = Line::from(vec![
        Span::raw(" "),
        Span::styled(label_text.to_string(), t.title()),
        Span::raw(" "),
        Span::styled(value, t.base()),
        Span::raw(" "),
    ]);

    let chart = Chart::new(datasets)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(t.border())
                .title(title),
        )
        .x_axis(Axis::default().bounds([xmin, xmax]))
        .y_axis(Axis::default().style(dim).bounds([0.0, ymax]).labels(vec![
            Span::styled(label(0.0), dim),
            Span::styled(label(ymax / 2.0), dim),
            Span::styled(label(ymax), dim),
        ]));
    f.render_widget(chart, area);
}

fn smoothed(hist: &[f64], alpha: f64) -> Vec<(f64, f64)> {
    let mut out = Vec::with_capacity(hist.len());
    let mut ema = 0.0;
    for (i, v) in hist.iter().enumerate() {
        ema = if i == 0 {
            *v
        } else {
            alpha * v + (1.0 - alpha) * ema
        };
        out.push((i as f64, ema));
    }
    out
}

/// Round an auto-scaled axis max up to 1/2/5 × 10^n (min 10) so the
/// labels stay readable and the axis doesn't jitter every tick.
fn nice_ceil(v: f64) -> f64 {
    if v <= 10.0 {
        return 10.0;
    }
    let mag = 10f64.powf(v.log10().floor());
    [1.0, 2.0, 5.0, 10.0]
        .into_iter()
        .map(|m| m * mag)
        .find(|&c| c >= v)
        .unwrap_or(10.0 * mag)
}

/// Green under load, red at >= 90% of the maximum.
fn load_color(t: &Theme, v: f64, max: f64) -> Color {
    let r = if max > 0.0 { v / max } else { 0.0 };
    if r >= 0.9 {
        t.palette.red
    } else {
        t.palette.green
    }
}

#[cfg(test)]
mod tests {
    use super::nice_ceil;

    #[test]
    fn nice_ceil_steps() {
        assert_eq!(nice_ceil(0.0), 10.0);
        assert_eq!(nice_ceil(12.0), 20.0);
        assert_eq!(nice_ceil(63.7), 100.0);
        assert_eq!(nice_ceil(41.0), 50.0);
        assert_eq!(nice_ceil(100.0), 100.0);
        assert_eq!(nice_ceil(130.0), 200.0);
    }
}
