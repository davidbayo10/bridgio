use std::collections::HashMap;
use std::sync::Arc;

use anyhow::{Result, anyhow};
use aws_sdk_sns::Client;
use aws_types::SdkConfig;
use tokio::sync::Semaphore;

use crate::models::{SqsSnsSubscription, SubscriptionInfo, TopicDetail, TopicInfo, name_from_arn};

pub struct SnsService {
    client: Client,
}

impl SnsService {
    pub fn new(config: &SdkConfig) -> Self {
        Self {
            client: Client::new(config),
        }
    }

    /// Lists all topics with basic attributes. Paginates automatically.
    pub async fn list_topics(&self) -> Result<Vec<TopicInfo>> {
        let mut topics: Vec<TopicInfo> = Vec::new();
        let mut next_token: Option<String> = None;

        loop {
            let mut req = self.client.list_topics();
            if let Some(token) = next_token {
                req = req.next_token(token);
            }

            let output = req
                .send()
                .await
                .map_err(|e| anyhow!("list_topics failed: {e}"))?;

            for topic in output.topics() {
                let arn = topic.topic_arn().unwrap_or_default().to_string();

                // Fetch topic attributes to get subscription count.
                let attr_output = self
                    .client
                    .get_topic_attributes()
                    .topic_arn(&arn)
                    .send()
                    .await
                    .map_err(|e| anyhow!("get_topic_attributes failed: {e}"))?;

                // attributes() returns Option<&HashMap<String,String>> in SDK v1.
                let subscriptions_confirmed = attr_output
                    .attributes()
                    .and_then(|m| m.get("SubscriptionsConfirmed"))
                    .and_then(|v: &String| v.parse().ok())
                    .unwrap_or(0u64);

                topics.push(TopicInfo {
                    name: name_from_arn(&arn),
                    arn,
                    subscriptions_confirmed,
                });
            }

            next_token = output.next_token().map(str::to_string);
            if next_token.is_none() {
                break;
            }
        }

        topics.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(topics)
    }

    /// Fetches full attributes and subscriptions for a topic (detail view).
    pub async fn get_topic_detail(&self, arn: &str) -> Result<TopicDetail> {
        // Attributes
        let attr_output = self
            .client
            .get_topic_attributes()
            .topic_arn(arn)
            .send()
            .await
            .map_err(|e| anyhow!("get_topic_attributes (detail) failed: {e}"))?;

        let empty = Default::default();
        let raw = attr_output.attributes().unwrap_or(&empty);
        let mut attributes: Vec<(String, String)> =
            raw.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        attributes.sort_by(|a, b| a.0.cmp(&b.0));

        // Subscriptions (paginated)
        let mut subscriptions: Vec<SubscriptionInfo> = Vec::new();
        let mut next_token: Option<String> = None;

        loop {
            let mut req = self.client.list_subscriptions_by_topic().topic_arn(arn);
            if let Some(token) = next_token {
                req = req.next_token(token);
            }

            let sub_output = req
                .send()
                .await
                .map_err(|e| anyhow!("list_subscriptions_by_topic failed: {e}"))?;

            for sub in sub_output.subscriptions() {
                subscriptions.push(SubscriptionInfo {
                    arn: sub.subscription_arn().unwrap_or("n/a").to_owned(),
                    protocol: sub.protocol().unwrap_or("n/a").to_string(),
                    endpoint: sub.endpoint().unwrap_or("n/a").to_string(),
                });
            }

            next_token = sub_output.next_token().map(str::to_string);
            if next_token.is_none() {
                break;
            }
        }

        Ok(TopicDetail {
            name: name_from_arn(arn),
            arn: arn.to_string(),
            attributes,
            subscriptions,
        })
    }

    /// Lists all SNS subscriptions with protocol "sqs" and returns a map
    /// from queue ARN → subscriptions (with filter policies).
    pub async fn list_sqs_subscriptions(&self) -> Result<HashMap<String, Vec<SqsSnsSubscription>>> {
        // ── Phase 1: collect all sqs subscriptions via list_subscriptions ──
        struct RawSub {
            queue_arn: String,
            topic_arn: String,
            subscription_arn: String,
        }

        let mut raw: Vec<RawSub> = Vec::new();
        let mut next_token: Option<String> = None;

        loop {
            let mut req = self.client.list_subscriptions();
            if let Some(token) = next_token {
                req = req.next_token(token);
            }
            let output = req
                .send()
                .await
                .map_err(|e| anyhow!("list_subscriptions failed: {e}"))?;

            for sub in output.subscriptions() {
                if sub.protocol().unwrap_or("") != "sqs" {
                    continue;
                }
                let queue_arn = sub.endpoint().unwrap_or("").to_string();
                let topic_arn = sub.topic_arn().unwrap_or("").to_string();
                let subscription_arn = sub.subscription_arn().unwrap_or("n/a").to_owned();
                if queue_arn.is_empty() || topic_arn.is_empty() {
                    continue;
                }
                raw.push(RawSub {
                    queue_arn,
                    topic_arn,
                    subscription_arn,
                });
            }

            next_token = output.next_token().map(str::to_string);
            if next_token.is_none() {
                break;
            }
        }

        // ── Phase 2: fetch subscription attributes concurrently ─────────────
        let sem = Arc::new(Semaphore::new(20));
        type FilterSet = tokio::task::JoinSet<Result<(String, String, String, Option<String>)>>;
        let mut set: FilterSet = tokio::task::JoinSet::new();

        for r in raw {
            let client = self.client.clone();
            let sem = Arc::clone(&sem);
            let sub_arn = r.subscription_arn.clone();
            let queue_arn = r.queue_arn.clone();
            let topic_arn = r.topic_arn.clone();
            set.spawn(async move {
                let _permit = sem.acquire().await.unwrap();

                // Subscriptions in "PendingConfirmation" have a placeholder ARN.
                let filter_policy = if sub_arn.starts_with("arn:") {
                    match client
                        .get_subscription_attributes()
                        .subscription_arn(&sub_arn)
                        .send()
                        .await
                    {
                        Ok(out) => out
                            .attributes()
                            .and_then(|m| m.get("FilterPolicy"))
                            .filter(|v| !v.is_empty())
                            .cloned(),
                        // Best-effort: ignore individual errors.
                        Err(_) => None,
                    }
                } else {
                    None
                };

                Ok((queue_arn, topic_arn, sub_arn, filter_policy))
            });
        }

        let mut map: HashMap<String, Vec<SqsSnsSubscription>> = HashMap::new();
        while let Some(res) = set.join_next().await {
            match res {
                Ok(Ok((queue_arn, topic_arn, subscription_arn, filter_policy))) => {
                    map.entry(queue_arn).or_default().push(SqsSnsSubscription {
                        topic_name: name_from_arn(&topic_arn),
                        topic_arn,
                        subscription_arn,
                        filter_policy,
                    });
                }
                Ok(Err(e)) => return Err(e),
                Err(e) => return Err(anyhow!("subscription attribute task panicked: {e}")),
            }
        }

        Ok(map)
    }
}
