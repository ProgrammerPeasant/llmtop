use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, Cell, Row, Table},
};

use crate::app::App;

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let header = Row::new(vec!["MODEL", "VRAM", "TOK/S", "POWER", "J/TOK", "$/1K*"])
        .style(Style::default().add_modifier(Modifier::BOLD));

    let total_power = app.hardware.as_ref().map(|h| h.power_w).unwrap_or(0.0);
    let active_models: Vec<_> = app.models.iter().filter(|m| m.tokens_per_sec > 0.1).collect();
    let share = if active_models.is_empty() {
        0.0
    } else {
        total_power / active_models.len() as f64
    };

    let rows: Vec<Row> = if app.models.is_empty() {
        vec![Row::new(vec![Cell::from("(no models loaded)")])]
    } else {
        app.models
            .iter()
            .map(|m| {
                let model_power = if m.tokens_per_sec > 0.1 { share } else { 0.0 };
                let j_per_tok = if m.tokens_per_sec > 0.1 {
                    model_power / m.tokens_per_sec
                } else {
                    0.0
                };
                let usd_per_1k = crate::pricing::provider_output_per_million(&app.cli.compare)
                    .map(|p| p / 1000.0)
                    .unwrap_or(0.0);
                Row::new(vec![
                    Cell::from(m.name.clone()),
                    Cell::from(format!("{:.1} GB", m.vram_mb as f64 / 1024.0)),
                    Cell::from(format!("{:>5.1}", m.tokens_per_sec)),
                    Cell::from(format!("{:>5.0} W", model_power)),
                    Cell::from(if j_per_tok > 0.0 {
                        format!("{:>5.1}", j_per_tok)
                    } else {
                        " idle".into()
                    }),
                    Cell::from(format!("{:>5.3}", usd_per_1k)),
                ])
            })
            .collect()
    };

    let title = match app.hardware.as_ref() {
        Some(h) if !h.gpu_name.is_empty() && h.vram_total_mb > 0 => format!(
            " llmtop · {} · {:.0} GB ",
            h.gpu_name,
            h.vram_total_mb as f64 / 1024.0
        ),
        _ => " llmtop ".into(),
    };

    let widths = [
        Constraint::Min(20),
        Constraint::Length(10),
        Constraint::Length(7),
        Constraint::Length(8),
        Constraint::Length(7),
        Constraint::Length(7),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(Block::default().borders(Borders::ALL).title(title));
    f.render_widget(table, area);
}
