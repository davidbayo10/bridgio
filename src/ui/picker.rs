use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Clear, List, ListItem, ListState},
};

use crate::app::App;
use crate::models::{AWS_REGIONS, View};
use crate::ui::theme::panel_block;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let is_profile = app.view == View::ProfilePicker;

    let items: Vec<&str> = if is_profile {
        app.profiles.iter().map(|s| s.as_str()).collect()
    } else {
        AWS_REGIONS.to_vec()
    };

    let title = if is_profile {
        " Select profile  (↑↓ / j k  Enter  Esc) "
    } else {
        " Select region   (↑↓ / j k  Enter  Esc) "
    };

    // Size the popup to fit the longest item + highlight symbol + border padding.
    let max_label = items.iter().map(|s| s.len()).max().unwrap_or(10) as u16;
    let title_width = title.len() as u16;
    let popup_w = (max_label.max(title_width) + 6).min(area.width.saturating_sub(4));
    let popup_h = (items.len() as u16 + 2).min(area.height.saturating_sub(2));

    let popup = centered_rect(popup_w, popup_h, area);

    // Clear the background so the list looks like a real popup.
    frame.render_widget(Clear, popup);

    let list_items: Vec<ListItem> = items.iter().map(|s| ListItem::new(*s)).collect();

    let list = List::new(list_items)
        .block(panel_block(Style::default().fg(Color::Cyan)).title(title))
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶ ");

    let mut state = ListState::default();
    state.select(Some(app.picker_cursor));

    frame.render_stateful_widget(list, popup, &mut state);
}

/// Returns a Rect of exactly `width × height` centred inside `area`.
fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let h_pad = area.width.saturating_sub(width) / 2;
    let v_pad = area.height.saturating_sub(height) / 2;

    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(v_pad),
            Constraint::Length(height),
            Constraint::Min(0),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(h_pad),
            Constraint::Length(width),
            Constraint::Min(0),
        ])
        .split(vert[1])[1]
}
