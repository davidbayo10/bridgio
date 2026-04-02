pub mod dep_map;
pub mod header;
pub mod help;
pub mod picker;
pub mod sns_detail;
pub mod sns_list;
pub mod sqs_detail;
pub mod sqs_list;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
};

use crate::app::App;
use crate::models::View;

/// Top-level render function — called every frame.
pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();

    // Split screen: header (3 rows) + content.
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    header::render(frame, chunks[0], app);

    match &app.view {
        View::SqsList | View::ProfilePicker | View::RegionPicker => {
            sqs_list::render(frame, chunks[1], app)
        }
        View::SqsDetail => sqs_detail::render(frame, chunks[1], app),
        View::SnsList => sns_list::render(frame, chunks[1], app),
        View::SnsDetail => sns_detail::render(frame, chunks[1], app),
        View::DependencyMap => dep_map::render(frame, chunks[1], app),
        View::Help => help::render(frame, area),
    }

    // Picker popups are rendered as overlays on top of everything.
    if matches!(app.view, View::ProfilePicker | View::RegionPicker) {
        picker::render(frame, area, app);
    }
}
