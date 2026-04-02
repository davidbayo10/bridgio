use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState},
};

use crate::app::App;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(3)])
        .split(area);

    render_table(frame, chunks[0], app);
    render_search_bar(frame, chunks[1], app);
}

fn render_table(frame: &mut Frame, area: Rect, app: &App) {
    let header = Row::new([
        Cell::from("Name").style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Cell::from("Subscriptions").style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Cell::from("ARN").style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
    ])
    .height(1)
    .bottom_margin(1);

    let topics = app.filtered_topics();

    // While loading and no stale data yet, render skeleton rows.
    let rows: Vec<Row> = if app.loading && app.topics.is_empty() {
        let ghost = Style::default().fg(Color::DarkGray);
        let skeletons = [
            ("░░░░░░░░░░░░░░░░░░░░░░", "░░", "░░░░░░░░░░░░░░░░░░░░░░░░░"),
            ("░░░░░░░░░░░░░░", "░░░", "░░░░░░░░░░░░░░░░░░"),
            ("░░░░░░░░░░░░░░░░░░", "░", "░░░░░░░░░░░░░░░░░░░░░░"),
            ("░░░░░░░░░░░░", "░░", "░░░░░░░░░░░░░░░"),
            ("░░░░░░░░░░░░░░░░░░░░", "░░░", "░░░░░░░░░░░░░░░░░░░░"),
        ];
        skeletons
            .iter()
            .map(|(n, s, a)| {
                Row::new([
                    Cell::from(*n).style(ghost),
                    Cell::from(*s).style(ghost),
                    Cell::from(*a).style(ghost),
                ])
            })
            .collect()
    } else {
        topics
            .iter()
            .map(|t| {
                let selected = app.selected_topics.contains(&t.arn);

                if selected {
                    let sel = Style::default()
                        .fg(Color::Black)
                        .bg(Color::Green)
                        .add_modifier(Modifier::BOLD);
                    Row::new([
                        Cell::from(format!("● {}", t.name)).style(sel),
                        Cell::from(t.subscriptions_confirmed.to_string()).style(sel),
                        Cell::from(t.arn.clone()).style(sel),
                    ])
                } else {
                    Row::new([
                        Cell::from(t.name.clone()),
                        Cell::from(t.subscriptions_confirmed.to_string()),
                        Cell::from(t.arn.clone()).style(Style::default().fg(Color::DarkGray)),
                    ])
                }
            })
            .collect()
    }; // end skeleton/real rows

    let spinner = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let spin_char = if app.loading {
        spinner[(app.loading_tick as usize / 2) % spinner.len()]
    } else {
        ""
    };
    let empty_hint = if app.loading && app.topics.is_empty() {
        format!(" {spin_char} loading…")
    } else if app.loading {
        format!(" {spin_char}")
    } else if topics.is_empty() && !app.search_query.is_empty() {
        " No results".to_string()
    } else if topics.is_empty() {
        " No topics found in this region".to_string()
    } else {
        String::new()
    };

    let widths = [
        Constraint::Percentage(35),
        Constraint::Length(14),
        Constraint::Percentage(65),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(
                    " SNS Topics ({}/{}){empty_hint} ",
                    topics.len(),
                    app.topics.len()
                ))
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .row_highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        );

    let mut state = TableState::default();
    if !topics.is_empty() {
        state.select(Some(app.list_cursor));
    }

    frame.render_stateful_widget(table, area, &mut state);
}

fn render_search_bar(frame: &mut Frame, area: Rect, app: &App) {
    let (text, border_style) = if app.search_active {
        (
            format!(" / {}█", app.search_query),
            Style::default().fg(Color::Yellow),
        )
    } else if !app.search_query.is_empty() {
        (
            format!(" / {}  (Esc clears)", app.search_query),
            Style::default().fg(Color::Yellow),
        )
    } else if !app.selected_topics.is_empty() {
        (
            format!(
                "  ● {} selected   [ Space ] toggle   [ m ] dependency map   [ x ] clear",
                app.selected_topics.len()
            ),
            Style::default().fg(Color::Green),
        )
    } else {
        (
            "  [ / ] search   [ Space ] select   [ F5 ] refresh".to_string(),
            Style::default().fg(Color::DarkGray),
        )
    };

    let para = Paragraph::new(Line::from(vec![Span::raw(text)])).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(border_style),
    );
    frame.render_widget(para, area);
}
