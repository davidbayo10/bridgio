use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Row, Table},
};

use crate::app::App;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let Some(detail) = &app.queue_detail else {
        let placeholder = Table::new(Vec::<Row>::new(), [Constraint::Percentage(100)]).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" SQS Queue Detail — Loading… ")
                .border_style(Style::default().fg(Color::DarkGray)),
        );
        frame.render_widget(placeholder, area);
        return;
    };

    // SNS subscriptions for this queue (from pre-loaded map), sorted by topic name.
    let mut sns_subs: Vec<_> = app
        .sqs_sns_map
        .get(&detail.arn)
        .map(|v| v.iter().collect())
        .unwrap_or_default();
    sns_subs.sort_by(|a, b| a.topic_name.cmp(&b.topic_name));

    // Reserve bottom 40 % for subscriptions when there are any, else use 20 %.
    let sub_pct = if sns_subs.is_empty() { 20 } else { 40 };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(100 - sub_pct),
            Constraint::Percentage(sub_pct),
        ])
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

    // ── Attributes ────────────────────────────────────────────────────────────
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

    let total = detail.attributes.len();
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
            " — {}/{total} (↑↓ j k)  Tab→subs",
            app.detail_scroll.min(total)
        )
    } else {
        format!(" — {total} attrs  Tab→attrs")
    };

    let attr_table = Table::new(
        attr_rows,
        [Constraint::Percentage(35), Constraint::Percentage(65)],
    )
    .header(attr_header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" ← SQS: {}{scroll_hint}", detail.name))
            .border_style(attr_border),
    );

    frame.render_widget(attr_table, chunks[0]);

    // ── SNS Subscriptions ────────────────────────────────────────────────────
    let sub_header = Row::new([
        Cell::from("Topic").style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Cell::from("Filter Policy").style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Cell::from("Subscription ARN").style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
    ])
    .height(1)
    .bottom_margin(1);

    let sub_total = sns_subs.len();
    let sub_rows: Vec<Row> = sns_subs
        .iter()
        .skip(app.sub_scroll)
        .map(|s| {
            let fp_cell = match s.filter_policy.as_deref() {
                Some(fp) => Cell::from(fp.to_string()).style(Style::default().fg(Color::Yellow)),
                None => Cell::from("none").style(Style::default().fg(Color::DarkGray)),
            };
            Row::new([
                Cell::from(s.topic_name.clone()).style(Style::default().fg(Color::White)),
                fp_cell,
                Cell::from(s.subscription_arn.clone()).style(Style::default().fg(Color::DarkGray)),
            ])
        })
        .collect();

    let sub_hint = if sns_subs.is_empty() {
        " — not subscribed to any topic".to_string()
    } else if app.detail_on_subs {
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
            Constraint::Percentage(20),
            Constraint::Percentage(45),
            Constraint::Percentage(35),
        ],
    )
    .header(sub_header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" SNS Subscriptions{sub_hint}"))
            .border_style(sub_border),
    );

    frame.render_widget(sub_table, chunks[1]);
}
