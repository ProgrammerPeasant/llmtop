use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    widgets::{Cell, Paragraph, Row, Table},
};

use crate::app::App;

/// Borderless model table: magenta bold header, foreground rows, idle rows
/// muted (as tui-tools dims an interface that is down), live tok/s in green.
pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let t = &app.theme;

    if app.models.is_empty() {
        let header = Row::new(COLUMNS.to_vec()).style(t.table_head());
        f.render_widget(Table::new(Vec::<Row>::new(), WIDTHS).header(header), area);
        let msg = area.height > 2;
        if msg {
            let body = Rect {
                y: area.y + 2,
                height: 1,
                ..area
            };
            f.render_widget(Paragraph::new(empty_text(app)).style(t.muted()), body);
        }
        return;
    }

    let total_power = app.hardware.as_ref().map(|h| h.power_w).unwrap_or(0.0);
    let active = app
        .models
        .iter()
        .filter(|r| r.info.tokens_per_sec > 0.1)
        .count();
    let share = if active == 0 {
        0.0
    } else {
        total_power / active as f64
    };
    let usd_per_1k = crate::pricing::provider_output_per_million(&app.cli.compare)
        .map(|p| p / 1000.0)
        .unwrap_or(0.0);

    let rows: Vec<Row> = app
        .models
        .iter()
        .map(|r| {
            let m = &r.info;
            let live = m.tokens_per_sec > 0.1;
            let vram = if r.source.reports_vram() {
                format!("{:.1}G", m.vram_mb as f64 / 1024.0)
            } else {
                "-".into()
            };
            let model_power = if live { share } else { 0.0 };
            let row_style = if live { t.base() } else { t.muted() };
            Row::new(vec![
                Cell::from(m.name.clone()),
                Cell::from(r.source.label()),
                Cell::from(vram),
                Cell::from(m.ctx_size.map(fmt_ctx).unwrap_or_else(|| "-".into())),
                Cell::from(format!("{:.1}", m.tokens_per_sec)).style(if live {
                    t.ok()
                } else {
                    row_style
                }),
                Cell::from(if live {
                    format!("{model_power:.0}W")
                } else {
                    "-".into()
                }),
                Cell::from(if live {
                    format!("{:.2}", model_power / m.tokens_per_sec)
                } else {
                    "idle".into()
                }),
                Cell::from(format!("{usd_per_1k:.3}")),
            ])
            .style(row_style)
        })
        .collect();

    let table = Table::new(rows, WIDTHS)
        .header(Row::new(COLUMNS.to_vec()).style(t.table_head()))
        .column_spacing(2);
    f.render_widget(table, area);
}

const COLUMNS: [&str; 8] = [
    "MODEL", "SRC", "VRAM", "CTX", "TOK/S", "POWER", "J/TOK", "$/1K",
];

const WIDTHS: [Constraint; 8] = [
    Constraint::Min(20),
    Constraint::Length(9),
    Constraint::Length(6),
    Constraint::Length(5),
    Constraint::Length(6),
    Constraint::Length(5),
    Constraint::Length(5),
    Constraint::Length(5),
];

/// Empty-state hint: name the backends being polled so a blank table is
/// self-explanatory (e.g. llama-server not running, or `--no-ollama`).
fn empty_text(app: &App) -> String {
    match super::backends(app).as_slice() {
        [] => "no backends enabled: see --llamacpp-url".into(),
        b => format!("no models loaded  ·  polling {}", b.join(", ")),
    }
}

/// Context window in tokens, compact: 4096 -> 4K, 131072 -> 128K, 1048576 -> 1M.
fn fmt_ctx(n: u64) -> String {
    if n >= 1 << 20 && n.is_multiple_of(1 << 20) {
        format!("{}M", n >> 20)
    } else if n >= 1024 {
        format!("{}K", (n as f64 / 1024.0).round() as u64)
    } else {
        n.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::fmt_ctx;

    #[test]
    fn ctx_formatting() {
        assert_eq!(fmt_ctx(512), "512");
        assert_eq!(fmt_ctx(4096), "4K");
        assert_eq!(fmt_ctx(262144), "256K");
        assert_eq!(fmt_ctx(1 << 20), "1M");
    }
}
