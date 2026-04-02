use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::app::App;
use crate::models::name_from_arn;

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    // Split: map content + hint bar at the bottom.
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(3)])
        .split(area);

    render_map(frame, chunks[0], app);
    render_hint(frame, chunks[1]);
}

fn render_map(frame: &mut Frame, area: Rect, app: &App) {
    // ── Build the edge set ──────────────────────────────────────────────────
    //
    // We track all SNS topics that appear in the selection (either selected
    // directly or referenced by a selected queue's subscription), and for each
    // topic which queues subscribe to it (intersected with selected queues or
    // all queues subscribing when the topic itself is selected).
    //
    // Reverse the sqs_sns_map: topic_arn → Vec<queue_arn>
    let mut topic_to_queues: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for (queue_arn, subs) in &app.sqs_sns_map {
        for sub in subs {
            topic_to_queues
                .entry(sub.topic_arn.clone())
                .or_default()
                .push(queue_arn.clone());
        }
    }

    // Collect edges: (topic_arn, topic_name, queue_arns_involved).
    // A topic is "involved" if:
    //   a) it is in selected_topics, OR
    //   b) at least one selected queue subscribes to it.
    let mut involved_topics: std::collections::BTreeMap<String, String> =
        std::collections::BTreeMap::new(); // topic_arn → topic_name

    // From selected queues: gather all their subscribed topics.
    for queue_arn in &app.selected_queues {
        if let Some(subs) = app.sqs_sns_map.get(queue_arn) {
            for sub in subs {
                involved_topics
                    .entry(sub.topic_arn.clone())
                    .or_insert_with(|| sub.topic_name.clone());
            }
        }
    }
    // From selected topics directly.
    for topic_arn in &app.selected_topics {
        involved_topics
            .entry(topic_arn.clone())
            .or_insert_with(|| name_from_arn(topic_arn));
    }

    // For each involved topic, which queues are "in scope":
    //   - all selected queues that subscribe to this topic, PLUS
    //   - if the topic itself is selected, all queues that subscribe to it
    //     (even if not individually selected).
    let mut lines: Vec<Line> = Vec::new();

    let sel_q = app.selected_queues.len();
    let sel_t = app.selected_topics.len();
    let header_text = format!(
        " Dependency Map — {} queue{} + {} topic{} selected ",
        sel_q,
        if sel_q == 1 { "" } else { "s" },
        sel_t,
        if sel_t == 1 { "" } else { "s" },
    );
    lines.push(Line::from(Span::styled(
        header_text,
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    // Track queues that have at least one topic connection (to find orphan queues later).
    let mut queues_with_topic: std::collections::HashSet<String> = std::collections::HashSet::new();

    for (topic_arn, topic_name) in &involved_topics {
        let topic_selected = app.selected_topics.contains(topic_arn);

        // Determine which queue ARNs to show under this topic.
        let mut queue_arns: Vec<String> = Vec::new();
        if let Some(all_q) = topic_to_queues.get(topic_arn) {
            for q_arn in all_q {
                let show = app.selected_queues.contains(q_arn)
                    || (topic_selected && !app.selected_queues.is_empty());
                // If the topic is selected, show ALL subscribing queues.
                // If only queues are selected, show only the ones in the selection.
                if topic_selected || app.selected_queues.contains(q_arn) {
                    queue_arns.push(q_arn.clone());
                    let _ = show;
                }
            }
        }
        queue_arns.sort();

        // Topic line.
        let topic_marker = if topic_selected { "★ SNS" } else { "  SNS" };
        let topic_style = if topic_selected {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        lines.push(Line::from(vec![
            Span::styled(
                format!("{topic_marker} "),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(topic_name.clone(), topic_style),
            Span::styled(
                format!("  [{}]", topic_arn),
                Style::default().fg(Color::DarkGray),
            ),
        ]));

        if queue_arns.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("       └── ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    "(no subscribing queues in selection)",
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        } else {
            let last = queue_arns.len() - 1;
            for (i, q_arn) in queue_arns.iter().enumerate() {
                let connector = if i == last {
                    "└──▶"
                } else {
                    "├──▶"
                };
                let q_name = name_from_arn(q_arn);
                let q_selected = app.selected_queues.contains(q_arn);
                let q_marker = if q_selected { "★ SQS" } else { "  SQS" };
                let q_style = if q_selected {
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("       {connector} {q_marker} "),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(q_name, q_style),
                    Span::styled(
                        format!("  [{}]", q_arn),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]));

                // Filter policy for this specific topic→queue edge.
                let filter_policy = app
                    .sqs_sns_map
                    .get(q_arn)
                    .and_then(|subs| subs.iter().find(|s| s.topic_arn == *topic_arn))
                    .and_then(|s| s.filter_policy.as_deref());

                let indent = if i == last {
                    "              "
                } else {
                    "       │      "
                };
                match filter_policy {
                    Some(fp) => {
                        lines.push(Line::from(vec![
                            Span::styled(
                                format!("{indent}⌬ filter: "),
                                Style::default().fg(Color::Yellow),
                            ),
                            Span::styled(fp.to_string(), Style::default().fg(Color::White)),
                        ]));
                    }
                    None => {
                        lines.push(Line::from(vec![
                            Span::styled(
                                format!("{indent}⌬ filter: "),
                                Style::default().fg(Color::DarkGray),
                            ),
                            Span::styled(
                                "none (receives all messages)",
                                Style::default().fg(Color::DarkGray),
                            ),
                        ]));
                    }
                }

                queues_with_topic.insert(q_arn.clone());
            }
        }
        lines.push(Line::from(""));
    }

    // ── Selected queues with no topic connections ───────────────────────────
    let orphan_queues: Vec<&String> = app
        .selected_queues
        .iter()
        .filter(|arn| !queues_with_topic.contains(*arn))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();

    if !orphan_queues.is_empty() {
        lines.push(Line::from(Span::styled(
            "  SQS queues with no SNS subscriptions:",
            Style::default().fg(Color::DarkGray),
        )));
        for q_arn in orphan_queues {
            let q_name = name_from_arn(q_arn);
            lines.push(Line::from(vec![
                Span::styled("  ★ SQS ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    q_name,
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("  [{}]", q_arn),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }
        lines.push(Line::from(""));
    }

    // ── Render with scroll ──────────────────────────────────────────────────
    let total_lines = lines.len();
    let visible = area.height.saturating_sub(2) as usize; // subtract borders
    let scroll = app.dep_scroll.min(total_lines.saturating_sub(visible));

    let visible_lines: Vec<Line> = lines.into_iter().skip(scroll).collect();

    let scroll_hint = if total_lines > visible + scroll {
        format!(" [{}/{}]", scroll + visible, total_lines)
    } else {
        String::new()
    };

    let para = Paragraph::new(visible_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" Dependency Map{scroll_hint} "))
            .border_style(Style::default().fg(Color::Cyan)),
    );

    frame.render_widget(para, area);
}

fn render_hint(frame: &mut Frame, area: Rect) {
    let para = Paragraph::new(Line::from(Span::raw(
        "  [ ↑↓ / j k ] scroll   [ m / Esc ] close   [ x ] clear selection",
    )))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(para, area);
}
