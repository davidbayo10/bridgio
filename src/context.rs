use std::collections::{BTreeMap, BTreeSet};

use crate::app::App;
use crate::models::{View, name_from_arn};

/// Builds a structured, human-readable (and agent-friendly) context string
/// for the current view. Returns `None` if there is nothing useful to export.
pub fn build(app: &App) -> Option<String> {
    match app.view {
        View::SqsList => Some(context_sqs_list(app)),
        View::SnsList => Some(context_sns_list(app)),
        View::SqsDetail => context_sqs_detail(app),
        View::SnsDetail => context_sns_detail(app),
        View::DependencyMap => Some(context_dep_map(app)),
        // Pickers / Help have no meaningful exportable context.
        View::ProfilePicker | View::RegionPicker | View::Help => None,
    }
}

// ── SQS list ────────────────────────────────────────────────────────────────

fn context_sqs_list(app: &App) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "# SQS Queues — profile: {} / region: {}\n\n",
        app.current_profile(),
        app.current_region()
    ));

    let queues = if app.selected_queues.is_empty() {
        app.filtered_queues()
    } else {
        app.filtered_queues()
            .into_iter()
            .filter(|q| app.selected_queues.contains(&q.arn))
            .collect()
    };

    let label = if app.selected_queues.is_empty() {
        format!("All queues ({})", queues.len())
    } else {
        format!("Selected queues ({})", queues.len())
    };

    out.push_str(&format!("## {label}\n\n"));

    for q in &queues {
        let subs: Vec<String> = app
            .sqs_sns_map
            .get(&q.arn)
            .map(|v| v.iter().map(|s| s.topic_name.clone()).collect())
            .unwrap_or_default();

        out.push_str(&format!("### {}\n", q.name));
        out.push_str(&format!("- ARN: {}\n", q.arn));
        out.push_str(&format!(
            "- Messages: {}  |  In-flight: {}  |  Delayed: {}\n",
            q.approx_messages, q.approx_messages_not_visible, q.approx_messages_delayed
        ));
        if subs.is_empty() {
            out.push_str("- SNS Subscriptions: none\n");
        } else {
            out.push_str(&format!(
                "- SNS Subscriptions ({}): {}\n",
                subs.len(),
                subs.join(", ")
            ));
        }
        out.push('\n');
    }

    out
}

// ── SNS list ─────────────────────────────────────────────────────────────────

fn context_sns_list(app: &App) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "# SNS Topics — profile: {} / region: {}\n\n",
        app.current_profile(),
        app.current_region()
    ));

    let topics = if app.selected_topics.is_empty() {
        app.filtered_topics()
    } else {
        app.filtered_topics()
            .into_iter()
            .filter(|t| app.selected_topics.contains(&t.arn))
            .collect()
    };

    let label = if app.selected_topics.is_empty() {
        format!("All topics ({})", topics.len())
    } else {
        format!("Selected topics ({})", topics.len())
    };

    out.push_str(&format!("## {label}\n\n"));

    for t in &topics {
        out.push_str(&format!("### {}\n", t.name));
        out.push_str(&format!("- ARN: {}\n", t.arn));
        out.push_str(&format!(
            "- Confirmed subscriptions: {}\n",
            t.subscriptions_confirmed
        ));
        out.push('\n');
    }

    out
}

// ── SQS detail ───────────────────────────────────────────────────────────────

fn context_sqs_detail(app: &App) -> Option<String> {
    let detail = app.queue_detail.as_ref()?;
    let mut out = String::new();

    out.push_str(&format!("# SQS Queue Detail — {}\n\n", detail.name));
    out.push_str(&format!("- Profile: {}\n", app.current_profile()));
    out.push_str(&format!("- Region:  {}\n", app.current_region()));
    out.push_str(&format!("- ARN:     {}\n", detail.arn));
    out.push('\n');

    out.push_str("## Attributes\n\n");
    for (k, v) in &detail.attributes {
        out.push_str(&format!("- {k}: {v}\n"));
    }
    out.push('\n');

    let subs = app.sqs_sns_map.get(&detail.arn);
    let subs_slice = subs.map(|v| v.as_slice()).unwrap_or(&[]);
    if subs_slice.is_empty() {
        out.push_str("## SNS Subscriptions\n\nnone\n");
    } else {
        out.push_str(&format!("## SNS Subscriptions ({})\n\n", subs_slice.len()));
        let mut sorted = subs_slice.to_vec();
        sorted.sort_by(|a, b| a.topic_name.cmp(&b.topic_name));
        for s in &sorted {
            let fp = s.filter_policy.as_deref().unwrap_or("none");
            out.push_str(&format!(
                "- Topic: {}  |  Filter Policy: {}  |  Subscription ARN: {}\n",
                s.topic_name, fp, s.subscription_arn
            ));
        }
    }

    Some(out)
}

// ── SNS detail ───────────────────────────────────────────────────────────────

fn context_sns_detail(app: &App) -> Option<String> {
    let detail = app.topic_detail.as_ref()?;
    let mut out = String::new();

    out.push_str(&format!("# SNS Topic Detail — {}\n\n", detail.name));
    out.push_str(&format!("- Profile: {}\n", app.current_profile()));
    out.push_str(&format!("- Region:  {}\n", app.current_region()));
    out.push_str(&format!("- ARN:     {}\n\n", detail.arn));

    out.push_str("## Attributes\n\n");
    for (k, v) in &detail.attributes {
        out.push_str(&format!("- {k}: {v}\n"));
    }
    out.push('\n');

    if detail.subscriptions.is_empty() {
        out.push_str("## Subscriptions\n\nnone\n");
    } else {
        out.push_str(&format!(
            "## Subscriptions ({})\n\n",
            detail.subscriptions.len()
        ));
        for s in &detail.subscriptions {
            out.push_str(&format!(
                "- Protocol: {}  |  Endpoint: {}  |  ARN: {}\n",
                s.protocol, s.endpoint, s.arn
            ));
        }
    }

    Some(out)
}

// ── Dependency map ───────────────────────────────────────────────────────────

fn context_dep_map(app: &App) -> String {
    let mut out = String::new();

    out.push_str("# Dependency Map\n\n");
    out.push_str(&format!(
        "- Profile: {}\n- Region:  {}\n",
        app.current_profile(),
        app.current_region()
    ));
    out.push_str(&format!(
        "- Selected: {} queue(s), {} topic(s)\n\n",
        app.selected_queues.len(),
        app.selected_topics.len()
    ));

    // Reverse map: topic_arn → Vec<queue_arn>
    let mut topic_to_queues: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (queue_arn, subs) in &app.sqs_sns_map {
        for sub in subs {
            topic_to_queues
                .entry(sub.topic_arn.clone())
                .or_default()
                .push(queue_arn.clone());
        }
    }

    // Involved topics (same logic as dep_map renderer).
    let mut involved_topics: BTreeMap<String, String> = BTreeMap::new();
    for queue_arn in &app.selected_queues {
        if let Some(subs) = app.sqs_sns_map.get(queue_arn) {
            for sub in subs {
                involved_topics
                    .entry(sub.topic_arn.clone())
                    .or_insert_with(|| sub.topic_name.clone());
            }
        }
    }
    for topic_arn in &app.selected_topics {
        involved_topics
            .entry(topic_arn.clone())
            .or_insert_with(|| name_from_arn(topic_arn));
    }

    if involved_topics.is_empty() && app.selected_queues.is_empty() {
        out.push_str("(no selection)\n");
        return out;
    }

    let mut queues_with_topic: BTreeSet<String> = BTreeSet::new();

    if !involved_topics.is_empty() {
        out.push_str("## SNS → SQS Relationships\n\n");

        for (topic_arn, topic_name) in &involved_topics {
            let topic_selected = app.selected_topics.contains(topic_arn);
            let marker = if topic_selected {
                "[SNS ★]"
            } else {
                "[SNS]  "
            };
            out.push_str(&format!("{marker} {topic_name}\n"));
            out.push_str(&format!("         ARN: {topic_arn}\n"));

            let mut queue_arns: Vec<String> = Vec::new();
            if let Some(all_q) = topic_to_queues.get(topic_arn) {
                for q_arn in all_q {
                    if topic_selected || app.selected_queues.contains(q_arn) {
                        queue_arns.push(q_arn.clone());
                    }
                }
            }
            queue_arns.sort();

            if queue_arns.is_empty() {
                out.push_str("         └── (no subscribing queues in selection)\n");
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
                    let q_marker = if q_selected { "[SQS ★]" } else { "[SQS]  " };
                    out.push_str(&format!("         {connector} {q_marker} {q_name}\n"));
                    out.push_str(&format!("                   ARN: {q_arn}\n"));
                    let fp = app
                        .sqs_sns_map
                        .get(q_arn)
                        .and_then(|subs| subs.iter().find(|s| s.topic_arn == *topic_arn))
                        .and_then(|s| s.filter_policy.as_deref())
                        .unwrap_or("none (receives all messages)");
                    out.push_str(&format!("                   filter: {fp}\n"));
                    queues_with_topic.insert(q_arn.clone());
                }
            }
            out.push('\n');
        }
    }

    // Orphan queues.
    let orphans: Vec<&String> = app
        .selected_queues
        .iter()
        .filter(|arn| !queues_with_topic.contains(*arn))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    if !orphans.is_empty() {
        out.push_str("## Isolated SQS Queues (no SNS subscriptions)\n\n");
        for q_arn in orphans {
            out.push_str(&format!(
                "- [SQS ★] {}  |  ARN: {}\n",
                name_from_arn(q_arn),
                q_arn
            ));
        }
        out.push('\n');
    }

    out
}
