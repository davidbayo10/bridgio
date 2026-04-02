use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Tabs},
};

use crate::app::App;
use crate::models::View;

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
        Span::styled("  ⟳ loading…", Style::default().fg(Color::Yellow))
    } else if let Some(err) = &app.status {
        Span::styled(
            format!("  ✗ {err}"),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
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
        Span::styled("profile: ", Style::default().fg(Color::DarkGray)),
        Span::styled(profile, Style::default().fg(Color::White)),
        Span::raw("  "),
        Span::styled("region: ", Style::default().fg(Color::DarkGray)),
        Span::styled(region, Style::default().fg(Color::Yellow)),
        status_span,
    ]);

    let para = Paragraph::new(line).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
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
    };

    let tabs = Tabs::new(titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .select(selected)
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .divider(Span::raw(" │ "));

    frame.render_widget(tabs, area);
}
