use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Tabs},
};

use crate::app::{App, StatusLevel};
use crate::models::View;
use crate::ui::theme::panel_block;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    // Split header into: [left info] [tabs]
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    render_info(frame, chunks[0], app);
    render_tabs(frame, chunks[1], app);
}

fn render_info(frame: &mut Frame, area: Rect, app: &App) {
    let profile = app.current_profile();
    let region = app.current_region();

    let status_span = if app.loading {
        Span::styled(
            "  ⟳ loading…",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
    } else if let Some(status) = &app.status {
        let (icon, color) = match status.level {
            StatusLevel::Info => ("•", Color::Cyan),
            StatusLevel::Success => ("✓", Color::Green),
            StatusLevel::Error => ("✗", Color::Red),
        };

        Span::styled(
            format!("  {icon} {}", status.text),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled("  ✓ ready", Style::default().fg(Color::Green))
    };

    let line = Line::from(vec![
        Span::styled(
            " bridgio ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("│ "),
        Span::styled("profile: ", Style::default().fg(Color::Gray)),
        Span::styled(profile, Style::default().fg(Color::White)),
        Span::raw("  "),
        Span::styled("region: ", Style::default().fg(Color::Gray)),
        Span::styled(region, Style::default().fg(Color::Yellow)),
        status_span,
    ]);

    let para = Paragraph::new(line).block(panel_block(Style::default().fg(Color::Gray)));
    frame.render_widget(para, area);
}

fn render_tabs(frame: &mut Frame, area: Rect, app: &App) {
    let titles = vec!["1: SQS", "2: SNS", "?: Help"];
    let selected = match app.view {
        View::SqsList | View::SqsDetail => 0,
        View::SnsList | View::SnsDetail => 1,
        View::Help => 2,
        // Pickers are overlays; keep the tab that was active before opening them.
        View::ProfilePicker | View::RegionPicker => match app.previous_view {
            View::SnsList | View::SnsDetail => 1,
            _ => 0,
        },
        View::DependencyMap => match app.previous_view {
            View::SnsList | View::SnsDetail => 1,
            _ => 0,
        },
        View::QuitConfirm => match app.quit_return_view.as_ref().unwrap_or(&View::SqsList) {
            View::SnsList | View::SnsDetail => 1,
            View::Help => 2,
            _ => 0,
        },
    };

    let tabs = Tabs::new(titles)
        .block(panel_block(Style::default().fg(Color::Gray)))
        .select(selected)
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .divider(Span::raw(" │ "));

    frame.render_widget(tabs, area);
}
