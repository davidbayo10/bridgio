use anyhow::{Result, anyhow};
use aws_sdk_cloudwatch::{
    Client,
    types::{Dimension, Metric, MetricDataQuery, MetricStat},
};
use aws_smithy_types::DateTime;
use aws_types::SdkConfig;
use std::time::{Duration, SystemTime};

use crate::models::{QueueCloudWatchMetrics, name_from_url};

const SQS_NAMESPACE: &str = "AWS/SQS";
const INSIGHT_WINDOW_SECS: u64 = 3600;
const METRIC_PERIOD_SECS: i32 = 300;

pub struct CloudWatchService {
    client: Client,
}

impl CloudWatchService {
    pub fn new(config: &SdkConfig) -> Self {
        Self {
            client: Client::new(config),
        }
    }

    pub async fn get_sqs_queue_metrics(&self, queue_url: &str) -> Result<QueueCloudWatchMetrics> {
        let queue_name = name_from_url(queue_url);
        let end = SystemTime::now();
        let start = end
            .checked_sub(Duration::from_secs(INSIGHT_WINDOW_SECS))
            .ok_or_else(|| anyhow!("failed to compute CloudWatch start time"))?;

        let dimension = Dimension::builder()
            .name("QueueName")
            .value(queue_name)
            .build();

        let queries = [
            build_query("sent", "NumberOfMessagesSent", "Sum", &dimension)?,
            build_query("received", "NumberOfMessagesReceived", "Sum", &dimension)?,
            build_query("deleted", "NumberOfMessagesDeleted", "Sum", &dimension)?,
            build_query("empty", "NumberOfEmptyReceives", "Sum", &dimension)?,
            build_query(
                "oldest",
                "ApproximateAgeOfOldestMessage",
                "Maximum",
                &dimension,
            )?,
        ];

        let output = self
            .client
            .get_metric_data()
            .set_metric_data_queries(Some(queries.to_vec()))
            .start_time(DateTime::from(start))
            .end_time(DateTime::from(end))
            .scan_by(aws_sdk_cloudwatch::types::ScanBy::TimestampAscending)
            .send()
            .await
            .map_err(|e| anyhow!("get_metric_data failed: {e}"))?;

        let mut metrics = QueueCloudWatchMetrics {
            window_secs: INSIGHT_WINDOW_SECS,
            ..QueueCloudWatchMetrics::default()
        };

        for result in output.metric_data_results() {
            let Some(id) = result.id() else {
                continue;
            };

            match id {
                "sent" => metrics.messages_sent = result.values().iter().copied().sum(),
                "received" => metrics.messages_received = result.values().iter().copied().sum(),
                "deleted" => metrics.messages_deleted = result.values().iter().copied().sum(),
                "empty" => metrics.empty_receives = result.values().iter().copied().sum(),
                "oldest" => {
                    metrics.oldest_message_age_secs =
                        result.values().iter().copied().reduce(f64::max);
                }
                _ => {}
            }
        }

        Ok(metrics)
    }
}

fn build_query(
    id: &str,
    metric_name: &str,
    stat: &str,
    dimension: &Dimension,
) -> Result<MetricDataQuery> {
    let metric = Metric::builder()
        .namespace(SQS_NAMESPACE)
        .metric_name(metric_name)
        .dimensions(dimension.clone())
        .build();

    let metric_stat = MetricStat::builder()
        .metric(metric)
        .period(METRIC_PERIOD_SECS)
        .stat(stat)
        .build();

    Ok(MetricDataQuery::builder()
        .id(id)
        .metric_stat(metric_stat)
        .return_data(true)
        .build())
}
