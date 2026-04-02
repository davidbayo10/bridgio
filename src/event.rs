use crossterm::event::{self, Event as CEvent, KeyEvent};
use std::collections::HashMap;
use std::time::Duration;
use tokio::sync::mpsc;

use crate::models::{QueueDetail, QueueInfo, SqsSnsSubscription, TopicDetail, TopicInfo};

/// All events that flow into the main application loop.
#[derive(Debug)]
pub enum AppEvent {
    /// A key was pressed.
    Key(KeyEvent),
    /// Periodic tick used to trigger background refresh checks.
    Tick,
    /// SQS queue list loaded successfully.
    SqsLoaded(Vec<QueueInfo>),
    /// SNS topic list loaded successfully.
    SnsLoaded(Vec<TopicInfo>),
    /// SQS queue detail loaded successfully.
    SqsDetailLoaded(QueueDetail),
    /// SNS topic detail loaded successfully.
    SnsDetailLoaded(TopicDetail),
    /// SNS→SQS subscription map loaded (keyed by queue ARN).
    SqsSnsMapLoaded(HashMap<String, Vec<SqsSnsSubscription>>),
    /// An error occurred (displayed in the status bar).
    Error(String),
}

/// Spawns a background task that polls terminal input events and sends them
/// over the returned channel. A periodic `Tick` is also sent so the app can
/// schedule refreshes even when no key is pressed.
pub fn start_event_handler(tick_rate: Duration) -> mpsc::UnboundedReceiver<AppEvent> {
    let (tx, rx) = mpsc::unbounded_channel();

    tokio::spawn(async move {
        loop {
            // Poll for a crossterm event within the tick window.
            match event::poll(tick_rate) {
                Ok(true) => {
                    if let Ok(CEvent::Key(key)) = event::read()
                        && tx.send(AppEvent::Key(key)).is_err()
                    {
                        break; // Receiver dropped – app is shutting down.
                    }
                }
                Ok(false) => {
                    // No event within tick_rate: send a Tick.
                    if tx.send(AppEvent::Tick).is_err() {
                        break;
                    }
                }
                Err(_) => {
                    if tx.send(AppEvent::Tick).is_err() {
                        break;
                    }
                }
            }
        }
    });

    rx
}
