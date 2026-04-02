use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, TableState},
};

use crate::app::App;
use crate::models::SortMode;

const HIGH_MSG_THRESHOLD: u64 = 1000;
const WARN_MSG_THRESHOLD: u64 = 100;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    // Split: table (fills available space) + search/hint bar (3 rows).
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(3)])
        .split(area);

    render_table(frame, chunks[0], app);
    render_search_bar(frame, chunks[1], app);
}

fn render_table(frame: &mut Frame, area: Rect, app: &App) {
    let sort_label = match app.sort_mode {
        SortMode::Name => "",
        SortMode::MessagesDesc => " [↓ msgs]",
        SortMode::MessagesAsc => " [↑ msgs]",
    };

    let header = Row::new([
        Cell::from("Name").style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Cell::from("Messages").style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Cell::from("In Flight").style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Cell::from("Delayed").style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Cell::from("SNS Subs").style(
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

    let queues = app.filtered_queues();

    // While loading and no stale data to show yet, render skeleton rows.
    let rows: Vec<Row> = if app.loading && app.queues.is_empty() {
        let ghost = Style::default().fg(Color::DarkGray);
        // Skeleton widths cycle to look like different-length content.
        let skeletons = [
            (
                "░░░░░░░░░░░░░░░░░░░░░░",
                "░░░",
                "░░░",
                "░░░",
                "░",
                "░░░░░░░░░░░░░░░░░░░░",
            ),
            (
                "░░░░░░░░░░░░░░",
                "░",
                "░░",
                "░░",
                "░░",
                "░░░░░░░░░░░░░░░░░░░░░░░░░",
            ),
            (
                "░░░░░░░░░░░░░░░░░░",
                "░░",
                "░",
                "░░░",
                "░",
                "░░░░░░░░░░░░░░░",
            ),
            (
                "░░░░░░░░░░░░",
                "░░░",
                "░░",
                "░",
                "░░",
                "░░░░░░░░░░░░░░░░░░░░░░",
            ),
            (
                "░░░░░░░░░░░░░░░░░░░░",
                "░",
                "░░░",
                "░░",
                "░",
                "░░░░░░░░░░░░░░░░░░",
            ),
        ];
        skeletons
            .iter()
            .map(|(n, m, f, d, s, a)| {
                Row::new([
                    Cell::from(*n).style(ghost),
                    Cell::from(*m).style(ghost),
                    Cell::from(*f).style(ghost),
                    Cell::from(*d).style(ghost),
                    Cell::from(*s).style(ghost),
                    Cell::from(*a).style(ghost),
                ])
            })
            .collect()
    } else {
        queues
            .iter()
            .map(|q| {
                let selected = app.selected_queues.contains(&q.arn);

                let msg_style = if q.approx_messages >= HIGH_MSG_THRESHOLD {
                    Style::default().fg(Color::Red)
                } else if q.approx_messages >= WARN_MSG_THRESHOLD {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default().fg(Color::White)
                };

                let sns_count = app.sqs_sns_map.get(&q.arn).map_or(0, |v| v.len());
                let sns_style = if sns_count > 0 {
                    Style::default().fg(Color::Cyan)
                } else {
                    Style::default().fg(Color::DarkGray)
                };

                let name_cell = if selected {
                    Cell::from(format!("● {}", q.name)).style(
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Green)
                            .add_modifier(Modifier::BOLD),
                    )
                } else {
                    Cell::from(q.name.clone())
                };

                let base_style = if selected {
                    Style::default().fg(Color::Black).bg(Color::Green)
                } else {
                    Style::default()
                };

                Row::new([
                    name_cell,
                    Cell::from(q.approx_messages.to_string()).style(if selected {
                        base_style
                    } else {
                        msg_style
                    }),
                    Cell::from(q.approx_messages_not_visible.to_string()).style(base_style),
                    Cell::from(q.approx_messages_delayed.to_string()).style(base_style),
                    Cell::from(sns_count.to_string()).style(if selected {
                        base_style
                    } else {
                        sns_style
                    }),
                    Cell::from(q.arn.rsplit(':').next().unwrap_or(&q.arn).to_string()).style(
                        if selected {
                            base_style
                        } else {
                            Style::default().fg(Color::DarkGray)
                        },
                    ),
                ])
            })
            .collect()
    }; // end skeleton/real rows

    let spinner = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let spin_char = if app.loading {
        spinner[(app.loading_tick as usize / 2) % spinner.len()]
    } else {
        ""
    };
    let empty_hint = if app.loading && app.queues.is_empty() {
        format!(" {spin_char} loading…")
    } else if app.loading {
        format!(" {spin_char}")
    } else if queues.is_empty() && !app.search_query.is_empty() {
        " No results".to_string()
    } else if queues.is_empty() {
        " No queues found in this region".to_string()
    } else {
        String::new()
    };

    let widths = [
        Constraint::Percentage(28),
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(9),
        Constraint::Length(9),
        Constraint::Percentage(44),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(
                    " SQS Queues ({}/{}){sort_label}{empty_hint} ",
                    queues.len(),
                    app.queues.len()
                ))
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .row_highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        );

    let mut state = TableState::default();
    if !queues.is_empty() {
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
    } else if !app.selected_queues.is_empty() {
        (
            format!(
                "  ● {} selected   [ Space ] toggle   [ m ] dependency map   [ x ] clear",
                app.selected_queues.len()
            ),
            Style::default().fg(Color::Green),
        )
    } else {
        (
            "  [ / ] search   [ s ] sort   [ Space ] select   [ F5 ] refresh".to_string(),
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
