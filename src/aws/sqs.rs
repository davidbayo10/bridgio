use std::sync::Arc;

use anyhow::{Result, anyhow};
use aws_sdk_sqs::{Client, types::QueueAttributeName};
use aws_types::SdkConfig;
use tokio::sync::Semaphore;

use crate::models::{QueueDetail, QueueInfo, name_from_url};

/// Maximum number of concurrent get_queue_attributes calls during list.
const MAX_CONCURRENT_ATTRS: usize = 20;

pub struct SqsService {
    client: Client,
}

impl SqsService {
    pub fn new(config: &SdkConfig) -> Self {
        Self {
            client: Client::new(config),
        }
    }

    /// Lists all queues with their key metrics.
    /// Queue URLs are collected via paginated list_queues, then all
    /// get_queue_attributes calls are fired concurrently (bounded by
    /// MAX_CONCURRENT_ATTRS) so the total time is ~one RTT instead of N×RTT.
    pub async fn list_queues(&self) -> Result<Vec<QueueInfo>> {
        let metric_attrs = vec![
            QueueAttributeName::ApproximateNumberOfMessages,
            QueueAttributeName::ApproximateNumberOfMessagesNotVisible,
            QueueAttributeName::ApproximateNumberOfMessagesDelayed,
            QueueAttributeName::QueueArn,
        ];

        // 1. Collect all queue URLs (cheap paginated call).
        let mut urls: Vec<String> = Vec::new();
        let mut next_token: Option<String> = None;
        loop {
            let mut req = self.client.list_queues();
            if let Some(token) = next_token {
                req = req.next_token(token);
            }
            let output = req
                .send()
                .await
                .map_err(|e| anyhow!("list_queues failed: {e}"))?;
            urls.extend(output.queue_urls().iter().map(|s| s.to_string()));
            next_token = output.next_token().map(str::to_string);
            if next_token.is_none() {
                break;
            }
        }

        // 2. Fetch attributes for every queue in parallel.
        let sem = Arc::new(Semaphore::new(MAX_CONCURRENT_ATTRS));
        let mut set: tokio::task::JoinSet<Result<QueueInfo>> = tokio::task::JoinSet::new();
        for url in urls {
            let client = self.client.clone();
            let attrs = metric_attrs.clone();
            let sem = Arc::clone(&sem);
            set.spawn(async move {
                let _permit = sem.acquire().await.unwrap();
                let attrs_output = client
                    .get_queue_attributes()
                    .queue_url(&url)
                    .set_attribute_names(Some(attrs))
                    .send()
                    .await
                    .map_err(|e| anyhow!("get_queue_attributes failed: {e}"))?;

                let empty = Default::default();
                let a = attrs_output.attributes().unwrap_or(&empty);
                let get = |key: &QueueAttributeName| -> u64 {
                    a.get(key)
                        .and_then(|v: &String| v.parse().ok())
                        .unwrap_or(0)
                };
                let arn = a
                    .get(&QueueAttributeName::QueueArn)
                    .cloned()
                    .unwrap_or_default();

                Ok(QueueInfo {
                    name: name_from_url(&url),
                    url,
                    arn,
                    approx_messages: get(&QueueAttributeName::ApproximateNumberOfMessages),
                    approx_messages_not_visible: get(
                        &QueueAttributeName::ApproximateNumberOfMessagesNotVisible,
                    ),
                    approx_messages_delayed: get(
                        &QueueAttributeName::ApproximateNumberOfMessagesDelayed,
                    ),
                })
            });
        }

        let mut queues: Vec<QueueInfo> = Vec::new();
        while let Some(res) = set.join_next().await {
            match res {
                Ok(Ok(q)) => queues.push(q),
                Ok(Err(e)) => return Err(e),
                Err(e) => return Err(anyhow!("queue attribute task panicked: {e}")),
            }
        }

        queues.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(queues)
    }

    /// Fetches all attributes for a specific queue (detail view).
    pub async fn get_queue_detail(&self, url: &str) -> Result<QueueDetail> {
        let output = self
            .client
            .get_queue_attributes()
            .queue_url(url)
            .attribute_names(QueueAttributeName::All)
            .send()
            .await
            .map_err(|e| anyhow!("get_queue_attributes (detail) failed: {e}"))?;

        let empty = Default::default();
        let raw = output.attributes().unwrap_or(&empty);
        let mut attributes: Vec<(String, String)> = raw
            .iter()
            .map(|(k, v)| (k.as_str().to_string(), v.clone()))
            .collect();
        attributes.sort_by(|a, b| a.0.cmp(&b.0));

        let arn = raw
            .get(&QueueAttributeName::QueueArn)
            .cloned()
            .unwrap_or_default();

        Ok(QueueDetail {
            name: name_from_url(url),
            arn,
            attributes,
        })
    }
}
