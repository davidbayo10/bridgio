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
}
