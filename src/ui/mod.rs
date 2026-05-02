mod footer;
mod sparkline;
mod table;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
};

use crate::app::App;

pub fn draw(f: &mut Frame, app: &App) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(6),    // models table
            Constraint::Length(7), // sparklines (3 lines each + borders)
            Constraint::Length(3), // session footer
        ])
        .split(area);

    table::draw(f, chunks[0], app);
    sparkline::draw(f, chunks[1], app);
    footer::draw(f, chunks[2], app);
}
