use crate::ui::theme::panel_block;
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Clear, Paragraph},
};

const HELP_TEXT: &[(&str, &str)] = &[
    ("q", "Open quit confirmation"),
    ("Ctrl+C", "Quit immediately"),
    ("1", "Switch to SQS view"),
    ("2", "Switch to SNS view"),
    ("↑ / k", "Move cursor up"),
    ("↓ / j", "Move cursor down"),
    ("Enter", "Open detail view"),
    ("Esc", "Back / cancel / close modal"),
    ("Space", "Select / deselect item in list"),
    ("m", "Open dependency map  (when items selected)"),
    ("x", "Clear all selections  (in dep. map)"),
    ("p / r", "Open profile / region picker"),
    ("Cmd+R", "Refresh now"),
    ("/", "Search by name  (* and ? supported)"),
    ("s", "Cycle sort: name → ↓msgs → ↑msgs  (SQS)"),
    ("Tab", "Switch focus between panels  (SQS detail)"),
    ("?", "Toggle this help"),
];

pub fn render(frame: &mut Frame, area: Rect) {
    // Centre a floating popup.
    let popup = centered_rect(50, 60, area);

    // Clear the background behind the popup.
    frame.render_widget(Clear, popup);

    let lines: Vec<Line> = HELP_TEXT
        .iter()
        .map(|(key, desc)| {
            Line::from(vec![
                Span::styled(
                    format!("{key:<20}"),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(*desc, Style::default().fg(Color::White)),
            ])
        })
        .collect();

    let para = Paragraph::new(lines)
        .block(
            panel_block(Style::default().fg(Color::Cyan))
                .title(" Keybindings — Esc / ? to close ")
                .title_alignment(Alignment::Center),
        )
        .alignment(Alignment::Left);

    frame.render_widget(para, popup);
}

/// Returns a centred rectangle with `percent_x` width and `percent_y` height
/// relative to the given `area`.
fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}
