use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Row, Table},
};

use crate::app::App;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let Some(detail) = &app.topic_detail else {
        let placeholder = Table::new(Vec::<Row>::new(), [Constraint::Percentage(100)]).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" SNS Topic Detail — Loading… ")
                .border_style(Style::default().fg(Color::DarkGray)),
        );
        frame.render_widget(placeholder, area);
        return;
    };

    // Split area into attributes (top 50%) and subscriptions (bottom 50%).
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    // Border colours: cyan = focused panel, dark gray = unfocused.
    let attr_border = if app.detail_on_subs {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default().fg(Color::Cyan)
    };
    let sub_border = if app.detail_on_subs {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    // ---- Attributes table ----
    let attr_header = Row::new([
        Cell::from("Attribute").style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Cell::from("Value").style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
    ])
    .height(1)
    .bottom_margin(1);

    let total_attrs = detail.attributes.len();
    let attr_rows: Vec<Row> = detail
        .attributes
        .iter()
        .skip(app.detail_scroll)
        .map(|(k, v)| {
            Row::new([
                Cell::from(k.clone()).style(Style::default().fg(Color::Yellow)),
                Cell::from(v.clone()),
            ])
        })
        .collect();

    let scroll_hint = if !app.detail_on_subs {
        format!(
            " — {}/{total_attrs} (↑↓ j k)  Tab→subs",
            app.detail_scroll.min(total_attrs)
        )
    } else {
        format!(" — {total_attrs} attrs  Tab→attrs")
    };

    let attr_table = Table::new(
        attr_rows,
        [Constraint::Percentage(35), Constraint::Percentage(65)],
    )
    .header(attr_header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" ← SNS: {} {scroll_hint}", detail.name))
            .border_style(attr_border),
    );

    frame.render_widget(attr_table, chunks[0]);

    // ---- Subscriptions table ----
    let sub_header = Row::new([
        Cell::from("Protocol").style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Cell::from("Endpoint").style(
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

    let sub_total = detail.subscriptions.len();
    let sub_rows: Vec<Row> = detail
        .subscriptions
        .iter()
        .skip(app.sub_scroll)
        .map(|s| {
            Row::new([
                Cell::from(s.protocol.clone()).style(Style::default().fg(Color::Yellow)),
                Cell::from(s.endpoint.clone()),
                Cell::from(s.arn.clone()).style(Style::default().fg(Color::DarkGray)),
            ])
        })
        .collect();

    let sub_hint = if app.detail_on_subs {
        format!(
            " {}/{sub_total} (↑↓ j k)  Tab→attrs",
            app.sub_scroll.min(sub_total)
        )
    } else {
        format!(" {sub_total}  Tab→focus")
    };

    let sub_table = Table::new(
        sub_rows,
        [
            Constraint::Length(12),
            Constraint::Percentage(50),
            Constraint::Percentage(50),
        ],
    )
    .header(sub_header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" Subscriptions{sub_hint}"))
            .border_style(sub_border),
    );

    frame.render_widget(sub_table, chunks[1]);
}
