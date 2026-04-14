/// Summary of a single SQS queue shown in the list view.
#[derive(Debug, Clone)]
pub struct QueueInfo {
    pub name: String,
    pub url: String,
    pub arn: String,
    pub approx_messages: u64,
    pub approx_messages_not_visible: u64,
    pub approx_messages_delayed: u64,
}

/// Full attribute set for the SQS detail view.
#[derive(Debug, Clone)]
pub struct QueueDetail {
    pub name: String,
    pub arn: String,
    pub attributes: Vec<(String, String)>,
}

impl QueueDetail {
    pub fn attribute(&self, key: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    pub fn attribute_u64(&self, key: &str) -> Option<u64> {
        self.attribute(key).and_then(|value| value.parse().ok())
    }
}

#[derive(Debug, Clone, Default)]
pub struct QueueCloudWatchMetrics {
    pub window_secs: u64,
    pub messages_sent: f64,
    pub messages_received: f64,
    pub messages_deleted: f64,
    pub empty_receives: f64,
    pub oldest_message_age_secs: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsightSeverity {
    Normal,
    Warning,
    Critical,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueInsight {
    pub state: String,
    pub detail: String,
    pub severity: InsightSeverity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueueInsights {
    pub drain_outlook: QueueInsight,
    pub time_to_empty: QueueInsight,
    pub completion_pressure: QueueInsight,
    pub oldest_message_risk: QueueInsight,
    pub processing_pressure: QueueInsight,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueueInsightsState {
    Loading,
    Ready(Box<QueueInsights>),
}

/// Summary of a single SNS topic shown in the list view.
#[derive(Debug, Clone)]
pub struct TopicInfo {
    pub name: String,
    pub arn: String,
    pub subscriptions_confirmed: u64,
}

/// Full attribute set + subscriptions for the SNS detail view.
#[derive(Debug, Clone)]
pub struct TopicDetail {
    pub name: String,
    pub arn: String,
    pub attributes: Vec<(String, String)>,
    pub subscriptions: Vec<SubscriptionInfo>,
}

/// One subscription entry under a SNS topic.
#[derive(Debug, Clone)]
pub struct SubscriptionInfo {
    pub arn: String,
    pub protocol: String,
    pub endpoint: String,
}

/// An SNS subscription whose endpoint is an SQS queue.
/// Stored in `App::sqs_sns_map` keyed by queue ARN.
#[derive(Debug, Clone)]
pub struct SqsSnsSubscription {
    pub topic_arn: String,
    pub topic_name: String,
    pub subscription_arn: String,
    /// Raw JSON filter policy, if one is set on the subscription.
    pub filter_policy: Option<String>,
}

/// Sort modes for the SQS queue list.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum SortMode {
    #[default]
    Name,
    MessagesDesc,
    MessagesAsc,
}

/// All state that the app can be showing at any time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum View {
    SqsList,
    SqsDetail,
    SnsList,
    SnsDetail,
    Help,
    ProfilePicker,
    RegionPicker,
    DependencyMap,
    QuitConfirm,
}

/// Well-known AWS regions offered in the region selector.
pub const AWS_REGIONS: &[&str] = &[
    "us-east-1",
    "us-east-2",
    "us-west-1",
    "us-west-2",
    "eu-west-1",
    "eu-west-2",
    "eu-west-3",
    "eu-central-1",
    "eu-north-1",
    "ap-southeast-1",
    "ap-southeast-2",
    "ap-northeast-1",
    "ap-northeast-2",
    "ap-south-1",
    "sa-east-1",
    "ca-central-1",
];

fn wildcard_match(pattern: &str, text: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let text: Vec<char> = text.chars().collect();

    let mut pat_idx = 0usize;
    let mut text_idx = 0usize;
    let mut star_idx: Option<usize> = None;
    let mut match_idx = 0usize;

    while text_idx < text.len() {
        if pat_idx < pattern.len()
            && (pattern[pat_idx] == '?' || pattern[pat_idx] == text[text_idx])
        {
            pat_idx += 1;
            text_idx += 1;
        } else if pat_idx < pattern.len() && pattern[pat_idx] == '*' {
            star_idx = Some(pat_idx);
            pat_idx += 1;
            match_idx = text_idx;
        } else if let Some(star) = star_idx {
            pat_idx = star + 1;
            match_idx += 1;
            text_idx = match_idx;
        } else {
            return false;
        }
    }

    while pat_idx < pattern.len() && pattern[pat_idx] == '*' {
        pat_idx += 1;
    }

    pat_idx == pattern.len()
}

/// Matches a user search query against a resource name.
///
/// Plain tokens behave like case-insensitive substrings.
/// Tokens containing `*` or `?` behave like shell-style wildcards.
/// Multiple space-separated tokens must all match.
pub fn matches_friendly_filter(query: &str, candidate: &str) -> bool {
    let query = query.trim();
    if query.is_empty() {
        return true;
    }

    let candidate = candidate.to_lowercase();

    query.split_whitespace().all(|raw_token| {
        let token = raw_token.to_lowercase();
        if token.contains('*') || token.contains('?') {
            wildcard_match(&token, &candidate)
        } else {
            candidate.contains(&token)
        }
    })
}

/// Converts a raw attribute key from the AWS API into a readable label.
#[allow(dead_code)]
pub fn pretty_attr_key(key: &str) -> String {
    key.replace("Approximate", "~")
        .chars()
        .flat_map(|c| {
            if c.is_uppercase() {
                vec![' ', c]
            } else {
                vec![c]
            }
        })
        .collect::<String>()
        .trim()
        .to_string()
}

/// Extracts the resource name from an ARN or URL.
pub fn name_from_arn(arn: &str) -> String {
    arn.rsplit(':').next().unwrap_or(arn).to_string()
}

/// Extracts the queue name from a queue URL.
pub fn name_from_url(url: &str) -> String {
    url.rsplit('/').next().unwrap_or(url).to_string()
}

pub fn compute_queue_insights(
    detail: &QueueDetail,
    metrics: Option<&QueueCloudWatchMetrics>,
) -> QueueInsights {
    let visible = detail
        .attribute_u64("ApproximateNumberOfMessages")
        .unwrap_or(0);
    let in_flight = detail
        .attribute_u64("ApproximateNumberOfMessagesNotVisible")
        .unwrap_or(0);
    let delayed = detail
        .attribute_u64("ApproximateNumberOfMessagesDelayed")
        .unwrap_or(0);
    let total_backlog = visible + in_flight + delayed;
    let retention_secs = detail.attribute_u64("MessageRetentionPeriod");

    let processing_pressure = compute_processing_pressure(in_flight, total_backlog);
    let Some(metrics) = metrics else {
        return QueueInsights {
            drain_outlook: unavailable_insight("CloudWatch metrics unavailable"),
            time_to_empty: unavailable_insight("Need sent/delete throughput"),
            completion_pressure: unavailable_insight("Need receive/delete throughput"),
            oldest_message_risk: unavailable_insight("Need oldest-message metric"),
            processing_pressure,
        };
    };

    let window_hours = metrics.window_secs as f64 / 3600.0;
    let sent = metrics.messages_sent.max(0.0);
    let received = metrics.messages_received.max(0.0);
    let deleted = metrics.messages_deleted.max(0.0);
    let tolerance = comparison_tolerance(sent.max(received).max(deleted));

    let drain_delta = deleted - sent;
    let drain_outlook = if drain_delta > tolerance {
        insight(
            InsightSeverity::Normal,
            format!(
                "Backlog is shrinking · sent {} / deleted {} in last {}",
                format_count(sent),
                format_count(deleted),
                format_window(metrics.window_secs)
            ),
        )
    } else if drain_delta < -tolerance {
        insight(
            InsightSeverity::Critical,
            format!(
                "Backlog is growing · sent {} / deleted {} in last {}",
                format_count(sent),
                format_count(deleted),
                format_window(metrics.window_secs)
            ),
        )
    } else {
        insight(
            InsightSeverity::Warning,
            format!(
                "Backlog is flat · sent {} / deleted {} in last {}",
                format_count(sent),
                format_count(deleted),
                format_window(metrics.window_secs)
            ),
        )
    };

    let net_deleted = drain_delta;
    let time_to_empty = if total_backlog == 0 {
        insight(
            InsightSeverity::Normal,
            "Queue is empty · no current backlog".to_string(),
        )
    } else if net_deleted < -tolerance {
        insight(
            InsightSeverity::Critical,
            format!(
                "Backlog cannot clear at current pace · backlog {} / net drain {} in last {}",
                total_backlog,
                format_signed_count(net_deleted),
                format_window(metrics.window_secs)
            ),
        )
    } else if net_deleted <= tolerance {
        insight(
            InsightSeverity::Warning,
            format!(
                "Backlog is not shrinking meaningfully · backlog {} / net drain {} in last {}",
                total_backlog,
                format_signed_count(net_deleted),
                format_window(metrics.window_secs)
            ),
        )
    } else {
        let net_deleted_per_hour = net_deleted / window_hours.max(1e-9);
        let eta_secs = total_backlog as f64 / net_deleted_per_hour * 3600.0;
        let mut eta_severity = if eta_secs > 2.0 * 3600.0 {
            InsightSeverity::Critical
        } else if eta_secs >= 30.0 * 60.0 {
            InsightSeverity::Warning
        } else {
            InsightSeverity::Normal
        };
        if retention_secs.is_some_and(|retention| eta_secs > retention as f64) {
            eta_severity = max_severity(eta_severity, InsightSeverity::Warning);
        }

        insight(
            eta_severity,
            format!(
                "Queue would clear in about {} · backlog {} / net drain {} per hour",
                format_duration(eta_secs),
                total_backlog,
                format_count(net_deleted_per_hour)
            ),
        )
    };

    let completion_pressure = if deleted - received > tolerance {
        completion_insight(
            InsightSeverity::Normal,
            "Consumers are completing messages faster than they receive them",
            received,
            deleted,
            metrics.empty_receives,
            metrics.window_secs,
        )
    } else if received - deleted > tolerance {
        completion_insight(
            InsightSeverity::Critical,
            "Consumers are falling behind on completions",
            received,
            deleted,
            metrics.empty_receives,
            metrics.window_secs,
        )
    } else {
        completion_insight(
            InsightSeverity::Warning,
            "Consumers are barely keeping up",
            received,
            deleted,
            metrics.empty_receives,
            metrics.window_secs,
        )
    };

    let oldest_message_risk = compute_oldest_message_risk(metrics.oldest_message_age_secs, retention_secs);

    QueueInsights {
        drain_outlook,
        time_to_empty,
        completion_pressure,
        oldest_message_risk,
        processing_pressure,
    }
}

fn completion_insight(
    severity: InsightSeverity,
    message: &str,
    received: f64,
    deleted: f64,
    empty_receives: f64,
    window_secs: u64,
) -> QueueInsight {
    let mut detail = format!(
        "{message} · received {} / deleted {} in last {}",
        format_count(received),
        format_count(deleted),
        format_window(window_secs)
    );

    if empty_receives > 0.0 {
        detail.push_str(&format!(" · empty receives {}", format_count(empty_receives)));
    }

    insight(severity, detail)
}

fn compute_processing_pressure(in_flight: u64, total_backlog: u64) -> QueueInsight {
    if total_backlog == 0 {
        return insight(
            InsightSeverity::Normal,
            "No current pressure · no backlog".to_string(),
        );
    }

    let ratio = in_flight as f64 / total_backlog as f64;
    let (severity, message) = if ratio > 0.9 {
        InsightSeverity::Critical
            .with_message("Nearly all backlog is stuck in-flight")
    } else if ratio >= 0.7 {
        InsightSeverity::Warning
            .with_message("A large share of backlog is already in-flight")
    } else {
        InsightSeverity::Normal
            .with_message("Most backlog is still available to process")
    };

    insight(
        severity,
        format!("{message} · {in_flight} in flight / {total_backlog} total"),
    )
}

fn compute_oldest_message_risk(
    oldest_age_secs: Option<f64>,
    retention_secs: Option<u64>,
) -> QueueInsight {
    let Some(age_secs) = oldest_age_secs else {
        return unavailable_insight("CloudWatch returned no oldest-message datapoint");
    };

    let absolute_severity = age_severity(age_secs);
    let retention_severity = retention_secs
        .filter(|retention| *retention > 0)
        .map(|retention| retention_age_severity(age_secs, retention));
    let severity = max_severity(
        absolute_severity,
        retention_severity.unwrap_or(InsightSeverity::Normal),
    );

    let message = match severity {
        InsightSeverity::Normal => "Messages are fresh",
        InsightSeverity::Warning => {
            if retention_severity == Some(InsightSeverity::Warning)
                && absolute_severity == InsightSeverity::Normal
            {
                "Messages are aging toward retention risk"
            } else {
                "Messages are aging longer than expected"
            }
        }
        InsightSeverity::Critical => {
            if retention_severity == Some(InsightSeverity::Critical)
                && absolute_severity != InsightSeverity::Critical
            {
                "Messages are close to expiry risk"
            } else {
                "Messages are aging too long"
            }
        }
        InsightSeverity::Unavailable => "Messages age is unavailable",
    };

    let mut detail = format!("{message} · oldest {}", format_duration(age_secs));

    if let Some(retention) = retention_secs.filter(|retention| *retention > 0)
        && retention_severity.unwrap_or(InsightSeverity::Normal) != InsightSeverity::Normal
    {
        detail.push_str(&format!(
            " · {:.0}% of retention {}",
            age_secs / retention as f64 * 100.0,
            format_duration(retention as f64)
        ));
    }

    insight(severity, detail)
}

fn insight(severity: InsightSeverity, detail: String) -> QueueInsight {
    QueueInsight {
        state: insight_state(severity).to_string(),
        detail,
        severity,
    }
}

fn insight_state(severity: InsightSeverity) -> &'static str {
    match severity {
        InsightSeverity::Normal => "Healthy",
        InsightSeverity::Warning => "Warning",
        InsightSeverity::Critical => "Critical",
        InsightSeverity::Unavailable => "Unavailable",
    }
}

fn age_severity(age_secs: f64) -> InsightSeverity {
    if age_secs >= 3600.0 {
        InsightSeverity::Critical
    } else if age_secs >= 1800.0 {
        InsightSeverity::Warning
    } else {
        InsightSeverity::Normal
    }
}

fn retention_age_severity(age_secs: f64, retention_secs: u64) -> InsightSeverity {
    let ratio = age_secs / retention_secs as f64;
    if ratio >= 0.8 {
        InsightSeverity::Critical
    } else if ratio >= 0.5 {
        InsightSeverity::Warning
    } else {
        InsightSeverity::Normal
    }
}

fn max_severity(left: InsightSeverity, right: InsightSeverity) -> InsightSeverity {
    if severity_rank(left) >= severity_rank(right) {
        left
    } else {
        right
    }
}

fn severity_rank(severity: InsightSeverity) -> u8 {
    match severity {
        InsightSeverity::Normal => 0,
        InsightSeverity::Warning => 1,
        InsightSeverity::Critical => 2,
        InsightSeverity::Unavailable => 3,
    }
}

fn unavailable_insight(detail: &str) -> QueueInsight {
    insight(InsightSeverity::Unavailable, detail.to_string())
}

fn comparison_tolerance(scale: f64) -> f64 {
    (scale * 0.05).max(3.0)
}

fn format_count(value: f64) -> String {
    format!("{:.0}", value.round())
}

fn format_signed_count(value: f64) -> String {
    if value >= 0.0 {
        format!("+{}", format_count(value))
    } else {
        format!("-{}", format_count(value.abs()))
    }
}

fn format_window(window_secs: u64) -> String {
    format_duration(window_secs as f64)
}

fn format_duration(total_secs: f64) -> String {
    let total_secs = total_secs.max(0.0).round() as u64;
    let days = total_secs / 86_400;
    let hours = (total_secs % 86_400) / 3_600;
    let minutes = (total_secs % 3_600) / 60;

    if days > 0 {
        format!("{days}d {hours}h")
    } else if hours > 0 {
        format!("{hours}h {minutes}m")
    } else if minutes > 0 {
        format!("{minutes}m")
    } else {
        format!("{total_secs}s")
    }
}

trait SeverityMessage {
    fn with_message(self, message: &'static str) -> (InsightSeverity, &'static str);
}

impl SeverityMessage for InsightSeverity {
    fn with_message(self, message: &'static str) -> (InsightSeverity, &'static str) {
        (self, message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_from_arn_extracts_last_segment() {
        assert_eq!(
            name_from_arn("arn:aws:sns:eu-west-1:123456789:my-topic"),
            "my-topic"
        );
    }

    #[test]
    fn name_from_arn_returns_input_when_no_colon() {
        assert_eq!(name_from_arn("no-colons-here"), "no-colons-here");
    }

    #[test]
    fn name_from_url_extracts_last_segment() {
        assert_eq!(
            name_from_url("https://sqs.eu-west-1.amazonaws.com/123456789/my-queue"),
            "my-queue"
        );
    }

    #[test]
    fn name_from_url_returns_input_when_no_slash() {
        assert_eq!(name_from_url("flat-name"), "flat-name");
    }

    #[test]
    fn pretty_attr_key_expands_camel_case() {
        let result = pretty_attr_key("VisibilityTimeout");
        assert!(result.contains("Visibility"));
        assert!(result.contains("Timeout"));
    }

    #[test]
    fn pretty_attr_key_replaces_approximate() {
        assert!(pretty_attr_key("ApproximateNumberOfMessages").contains('~'));
    }

    #[test]
    fn sort_mode_default_is_name() {
        assert_eq!(SortMode::default(), SortMode::Name);
    }

    #[test]
    fn friendly_filter_matches_plain_substrings_case_insensitively() {
        assert!(matches_friendly_filter("DLQ", "orders-dlq"));
    }

    #[test]
    fn friendly_filter_supports_star_wildcards() {
        assert!(matches_friendly_filter("*-dlq", "orders-dlq"));
        assert!(matches_friendly_filter("orders-*", "orders-dlq"));
        assert!(!matches_friendly_filter("*-dlq", "orders-main"));
    }

    #[test]
    fn friendly_filter_supports_question_wildcards() {
        assert!(matches_friendly_filter("orders-??q", "orders-dlq"));
        assert!(!matches_friendly_filter("orders-??q", "orders-dlqq"));
    }

    #[test]
    fn friendly_filter_requires_all_tokens_to_match() {
        assert!(matches_friendly_filter("orders *-dlq", "orders-dlq"));
        assert!(!matches_friendly_filter("orders *-fifo", "orders-dlq"));
    }

    #[test]
    fn queue_insights_report_growing_queue_without_eta() {
        let detail = QueueDetail {
            name: "orders".to_string(),
            arn: "arn:aws:sqs:eu-west-1:123456789012:orders".to_string(),
            attributes: vec![
                ("ApproximateNumberOfMessages".to_string(), "120".to_string()),
                (
                    "ApproximateNumberOfMessagesNotVisible".to_string(),
                    "30".to_string(),
                ),
                (
                    "ApproximateNumberOfMessagesDelayed".to_string(),
                    "10".to_string(),
                ),
                ("MessageRetentionPeriod".to_string(), "345600".to_string()),
            ],
        };
        let metrics = QueueCloudWatchMetrics {
            window_secs: 3600,
            messages_sent: 500.0,
            messages_received: 480.0,
            messages_deleted: 300.0,
            empty_receives: 10.0,
            oldest_message_age_secs: Some(3600.0),
        };

        let insights = compute_queue_insights(&detail, Some(&metrics));

        assert_eq!(insights.drain_outlook.state, "Critical");
        assert_eq!(insights.time_to_empty.state, "Critical");
        assert_eq!(insights.completion_pressure.state, "Critical");
    }

    #[test]
    fn queue_insights_report_finite_eta_when_queue_is_draining() {
        let detail = QueueDetail {
            name: "orders".to_string(),
            arn: "arn:aws:sqs:eu-west-1:123456789012:orders".to_string(),
            attributes: vec![
                ("ApproximateNumberOfMessages".to_string(), "60".to_string()),
                (
                    "ApproximateNumberOfMessagesNotVisible".to_string(),
                    "20".to_string(),
                ),
                (
                    "ApproximateNumberOfMessagesDelayed".to_string(),
                    "0".to_string(),
                ),
                ("MessageRetentionPeriod".to_string(), "345600".to_string()),
            ],
        };
        let metrics = QueueCloudWatchMetrics {
            window_secs: 3600,
            messages_sent: 100.0,
            messages_received: 120.0,
            messages_deleted: 220.0,
            empty_receives: 0.0,
            oldest_message_age_secs: Some(10_000.0),
        };

        let insights = compute_queue_insights(&detail, Some(&metrics));

        assert_eq!(insights.drain_outlook.state, "Healthy");
        assert_eq!(insights.time_to_empty.state, "Warning");
        assert_eq!(insights.completion_pressure.state, "Healthy");
    }

    #[test]
    fn queue_insights_surface_oldest_message_risk_and_idle_pressure() {
        let detail = QueueDetail {
            name: "orders".to_string(),
            arn: "arn:aws:sqs:eu-west-1:123456789012:orders".to_string(),
            attributes: vec![
                ("ApproximateNumberOfMessages".to_string(), "0".to_string()),
                (
                    "ApproximateNumberOfMessagesNotVisible".to_string(),
                    "0".to_string(),
                ),
                (
                    "ApproximateNumberOfMessagesDelayed".to_string(),
                    "0".to_string(),
                ),
                ("MessageRetentionPeriod".to_string(), "100".to_string()),
            ],
        };
        let metrics = QueueCloudWatchMetrics {
            window_secs: 3600,
            messages_sent: 0.0,
            messages_received: 0.0,
            messages_deleted: 0.0,
            empty_receives: 0.0,
            oldest_message_age_secs: Some(90.0),
        };

        let insights = compute_queue_insights(&detail, Some(&metrics));

        assert_eq!(insights.oldest_message_risk.state, "Critical");
        assert_eq!(insights.processing_pressure.state, "Healthy");
    }

    #[test]
    fn queue_insights_fall_back_to_partial_view_without_cloudwatch() {
        let detail = QueueDetail {
            name: "orders".to_string(),
            arn: "arn:aws:sqs:eu-west-1:123456789012:orders".to_string(),
            attributes: vec![
                ("ApproximateNumberOfMessages".to_string(), "10".to_string()),
                (
                    "ApproximateNumberOfMessagesNotVisible".to_string(),
                    "9".to_string(),
                ),
                (
                    "ApproximateNumberOfMessagesDelayed".to_string(),
                    "1".to_string(),
                ),
            ],
        };

        let insights = compute_queue_insights(&detail, None);

        assert_eq!(insights.drain_outlook.state, "Unavailable");
        assert_eq!(insights.processing_pressure.state, "Healthy");
    }

    #[test]
    fn queue_insights_flag_old_messages_even_with_long_retention() {
        let detail = QueueDetail {
            name: "orders".to_string(),
            arn: "arn:aws:sqs:eu-west-1:123456789012:orders".to_string(),
            attributes: vec![
                ("ApproximateNumberOfMessages".to_string(), "15".to_string()),
                (
                    "ApproximateNumberOfMessagesNotVisible".to_string(),
                    "3".to_string(),
                ),
                (
                    "ApproximateNumberOfMessagesDelayed".to_string(),
                    "0".to_string(),
                ),
                ("MessageRetentionPeriod".to_string(), "345600".to_string()),
            ],
        };
        let metrics = QueueCloudWatchMetrics {
            window_secs: 3600,
            messages_sent: 80.0,
            messages_received: 75.0,
            messages_deleted: 120.0,
            empty_receives: 0.0,
            oldest_message_age_secs: Some(89.0 * 60.0),
        };

        let insights = compute_queue_insights(&detail, Some(&metrics));

        assert_eq!(insights.oldest_message_risk.state, "Critical");
        assert!(insights.oldest_message_risk.detail.contains("Messages are aging too long"));
    }

    #[test]
    fn queue_insights_warn_when_backlog_is_nearly_flat() {
        let detail = QueueDetail {
            name: "orders".to_string(),
            arn: "arn:aws:sqs:eu-west-1:123456789012:orders".to_string(),
            attributes: vec![
                ("ApproximateNumberOfMessages".to_string(), "40".to_string()),
                (
                    "ApproximateNumberOfMessagesNotVisible".to_string(),
                    "10".to_string(),
                ),
                (
                    "ApproximateNumberOfMessagesDelayed".to_string(),
                    "0".to_string(),
                ),
                ("MessageRetentionPeriod".to_string(), "345600".to_string()),
            ],
        };
        let metrics = QueueCloudWatchMetrics {
            window_secs: 3600,
            messages_sent: 100.0,
            messages_received: 100.0,
            messages_deleted: 102.0,
            empty_receives: 0.0,
            oldest_message_age_secs: Some(600.0),
        };

        let insights = compute_queue_insights(&detail, Some(&metrics));

        assert_eq!(insights.drain_outlook.state, "Warning");
        assert_eq!(insights.time_to_empty.state, "Warning");
    }
}
