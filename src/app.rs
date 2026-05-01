use std::collections::{HashMap, HashSet};
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tokio::sync::mpsc;

use crate::aws::cloudwatch::CloudWatchService;
use crate::aws::config::load_sdk_config;
use crate::aws::sns::SnsService;
use crate::aws::sqs::SqsService;
use crate::event::{AppEvent, start_event_handler};
use crate::models::{
    AWS_REGIONS, QueueCloudWatchMetrics, QueueDetail, QueueInfo, QueueInsightsState, SortMode,
    SqsSnsSubscription, TopicDetail, TopicInfo, View, compute_queue_insights,
};
use crate::persist;
use crate::ui;

/// How often the event loop ticks when there is no input (ms).
const TICK_RATE_MS: u64 = 250;

/// How many ticks between automatic background refreshes (~30 s).
const AUTO_REFRESH_TICKS: u64 = 30_000 / TICK_RATE_MS;

/// Ticks to wait after the last config change before firing a refresh (~1 s).
const DEBOUNCE_TICKS: u64 = 4;

fn is_manual_refresh_key(key: crossterm::event::KeyEvent) -> bool {
    let allowed_modifiers = KeyModifiers::SUPER | KeyModifiers::SHIFT;
    matches!(key.code, KeyCode::Char('r') | KeyCode::Char('R'))
        && key.modifiers.contains(KeyModifiers::SUPER)
        && key.modifiers.difference(allowed_modifiers).is_empty()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusLevel {
    Info,
    Success,
    Error,
}

#[derive(Debug, Clone)]
pub struct StatusMessage {
    pub level: StatusLevel,
    pub text: String,
}

/// Central application state.
pub struct App {
    // Navigation
    pub view: View,
    pub previous_view: View,
    pub quit_return_view: Option<View>,

    // AWS connection settings
    pub profiles: Vec<String>,
    pub profile_idx: usize,
    pub region_idx: usize,

    // Data
    pub queues: Vec<QueueInfo>,
    pub topics: Vec<TopicInfo>,
    pub queue_detail: Option<QueueDetail>,
    pub queue_insights: Option<QueueInsightsState>,
    pub topic_detail: Option<TopicDetail>,
    /// SNS→SQS subscriptions, keyed by queue ARN.
    pub sqs_sns_map: HashMap<String, Vec<SqsSnsSubscription>>,

    // UI state
    pub list_cursor: usize,
    pub detail_scroll: usize,
    pub loading: bool,
    pub status: Option<StatusMessage>,

    // Detail panel focus (SQS detail only)
    /// When true, ↑↓/j/k scroll the SNS subscriptions panel instead of attributes.
    pub detail_on_subs: bool,
    pub sub_scroll: usize,

    // Picker popup cursor (ProfilePicker / RegionPicker)
    pub picker_cursor: usize,

    // Multi-selection (ARNs)
    pub selected_queues: HashSet<String>,
    pub selected_topics: HashSet<String>,

    // Dependency map scroll
    pub dep_scroll: usize,

    // Search & sort
    pub search_query: String,
    pub search_active: bool,
    pub sort_mode: SortMode,
    pub should_quit: bool,

    // Internal
    tick_counter: u64,
    pending_refresh: bool,
    debounce_ticks: u64,
    pending_requests: usize,
    active_sqs_queue_url: Option<String>,
    queue_cloudwatch_metrics: Option<Result<QueueCloudWatchMetrics, String>>,
    pub loading_tick: u64,
    pub event_tx: mpsc::UnboundedSender<AppEvent>,
}

impl App {
    pub fn new(profiles: Vec<String>, event_tx: mpsc::UnboundedSender<AppEvent>) -> Self {
        // Restore last-used profile and region, falling back to defaults.
        let (profile_idx, region_idx) =
            if let Some((saved_profile, saved_region)) = persist::load_state() {
                let pi = profiles
                    .iter()
                    .position(|p| p == &saved_profile)
                    .unwrap_or(0);
                let ri = AWS_REGIONS
                    .iter()
                    .position(|r| *r == saved_region)
                    .unwrap_or(4);
                (pi, ri)
            } else {
                (0, 4) // default: first profile, eu-west-1
            };

        Self {
            view: View::SqsList,
            previous_view: View::SqsList,
            quit_return_view: None,
            profiles,
            profile_idx,
            region_idx,
            queues: Vec::new(),
            topics: Vec::new(),
            queue_detail: None,
            queue_insights: None,
            topic_detail: None,
            sqs_sns_map: HashMap::new(),
            list_cursor: 0,
            detail_scroll: 0,
            loading: false,
            status: None,
            detail_on_subs: false,
            sub_scroll: 0,
            picker_cursor: 0,
            selected_queues: HashSet::new(),
            selected_topics: HashSet::new(),
            dep_scroll: 0,
            search_query: String::new(),
            search_active: false,
            sort_mode: SortMode::default(),
            should_quit: false,
            tick_counter: 0,
            pending_refresh: false,
            debounce_ticks: 0,
            pending_requests: 0,
            active_sqs_queue_url: None,
            queue_cloudwatch_metrics: None,
            loading_tick: 0,
            event_tx,
        }
    }

    pub fn current_profile(&self) -> &str {
        &self.profiles[self.profile_idx]
    }

    pub fn current_region(&self) -> &str {
        AWS_REGIONS[self.region_idx]
    }

    // -------------------------------------------------------------------
    // Filtered / sorted views (used by UI and open_detail)
    // -------------------------------------------------------------------

    /// Queues matching `search_query`, ordered by `sort_mode`.
    pub fn filtered_queues(&self) -> Vec<QueueInfo> {
        let mut result: Vec<QueueInfo> = self
            .queues
            .iter()
            .filter(|queue| crate::models::matches_friendly_filter(&self.search_query, &queue.name))
            .cloned()
            .collect();
        match self.sort_mode {
            SortMode::Name => {} // already sorted by name from AWS layer
            SortMode::MessagesDesc => {
                result.sort_by_key(|b| std::cmp::Reverse(b.approx_messages));
            }
            SortMode::MessagesAsc => {
                result.sort_by_key(|q| q.approx_messages);
            }
        }
        result
    }

    /// Topics matching `search_query` (always name-sorted).
    pub fn filtered_topics(&self) -> Vec<TopicInfo> {
        self.topics
            .iter()
            .filter(|t| crate::models::matches_friendly_filter(&self.search_query, &t.name))
            .cloned()
            .collect()
    }

    /// Number of items in the currently active filtered list.
    pub fn list_len(&self) -> usize {
        match self.view {
            View::SqsList => self.filtered_queues().len(),
            View::SnsList => self.filtered_topics().len(),
            _ => 0,
        }
    }

    // -------------------------------------------------------------------
    // Input handling
    // -------------------------------------------------------------------

    pub fn on_key(&mut self, key: crossterm::event::KeyEvent) {
        use KeyCode::*;

        if self.view == View::QuitConfirm {
            self.on_key_quit_confirm(key);
            return;
        }

        if is_manual_refresh_key(key) {
            self.trigger_manual_refresh();
            return;
        }

        // In search mode all printable chars feed the query.
        if self.search_active {
            match key.code {
                Esc => {
                    self.search_active = false;
                    self.search_query.clear();
                    self.list_cursor = 0;
                }
                Enter => {
                    self.search_active = false;
                }
                Backspace => {
                    self.search_query.pop();
                    self.list_cursor = 0;
                }
                Char(c) => {
                    self.search_query.push(c);
                    self.list_cursor = 0;
                }
                Up | Down => self.on_key_list(key),
                _ => {}
            }
            return;
        }

        if key.code == Char('q') {
            self.open_quit_confirm();
            return;
        }

        // 'c' copies context to clipboard from any view (except while typing in search).
        if key.code == Char('c') {
            match crate::context::build(self) {
                Some(text) => {
                    if crate::clipboard::copy(&text) {
                        self.set_status(StatusLevel::Success, "copied to clipboard");
                    } else {
                        self.set_status(StatusLevel::Error, "clipboard unavailable");
                    }
                }
                None => {
                    self.set_status(StatusLevel::Info, "nothing to copy in this view");
                }
            }
            return;
        }

        match &self.view {
            View::Help => self.on_key_help(key),
            View::ProfilePicker | View::RegionPicker => self.on_key_picker(key),
            View::SqsList | View::SnsList => self.on_key_list(key),
            View::SqsDetail | View::SnsDetail => self.on_key_detail(key),
            View::DependencyMap => self.on_key_dep_map(key),
            View::QuitConfirm => self.on_key_quit_confirm(key),
        }
    }

    fn on_key_list(&mut self, key: crossterm::event::KeyEvent) {
        use KeyCode::*;
        match key.code {
            // View switch (resets search)
            Char('1') if self.view != View::SqsList => {
                self.view = View::SqsList;
                self.list_cursor = 0;
                self.search_query.clear();
                self.search_active = false;
            }
            Char('2') if self.view != View::SnsList => {
                self.view = View::SnsList;
                self.list_cursor = 0;
                self.search_query.clear();
                self.search_active = false;
            }
            // Profile picker
            Char('p') | Char('P') => {
                self.previous_view = self.view.clone();
                self.picker_cursor = self.profile_idx;
                self.view = View::ProfilePicker;
            }
            // Region picker
            Char('r') | Char('R') => {
                self.previous_view = self.view.clone();
                self.picker_cursor = self.region_idx;
                self.view = View::RegionPicker;
            }
            // Navigation
            Up | Char('k') if self.list_cursor > 0 => {
                self.list_cursor -= 1;
            }
            Down | Char('j') if self.list_cursor + 1 < self.list_len() => {
                self.list_cursor += 1;
            }
            // Open detail
            Enter => self.open_detail(),
            // Search
            Char('/') => {
                self.search_active = true;
            }
            Esc if !self.search_query.is_empty() => {
                self.search_query.clear();
                self.list_cursor = 0;
            }
            // Sort (SQS only): Name → ↓msgs → ↑msgs → Name
            Char('s') if self.view == View::SqsList => {
                self.sort_mode = match self.sort_mode {
                    SortMode::Name => SortMode::MessagesDesc,
                    SortMode::MessagesDesc => SortMode::MessagesAsc,
                    SortMode::MessagesAsc => SortMode::Name,
                };
                self.list_cursor = 0;
            }
            // Toggle selection with Space
            Char(' ') => match self.view {
                View::SqsList => {
                    let queues = self.filtered_queues();
                    if let Some(q) = queues.get(self.list_cursor) {
                        if self.selected_queues.contains(&q.arn) {
                            self.selected_queues.remove(&q.arn);
                        } else {
                            self.selected_queues.insert(q.arn.clone());
                        }
                    }
                }
                View::SnsList => {
                    let topics = self.filtered_topics();
                    if let Some(t) = topics.get(self.list_cursor) {
                        if self.selected_topics.contains(&t.arn) {
                            self.selected_topics.remove(&t.arn);
                        } else {
                            self.selected_topics.insert(t.arn.clone());
                        }
                    }
                }
                _ => {}
            },
            // Open dependency map
            Char('m') if (!self.selected_queues.is_empty() || !self.selected_topics.is_empty()) => {
                self.previous_view = self.view.clone();
                self.dep_scroll = 0;
                self.view = View::DependencyMap;
            }
            // Clear all selections
            Char('x') => {
                self.selected_queues.clear();
                self.selected_topics.clear();
            }
            // Help
            Char('?') => {
                self.previous_view = self.view.clone();
                self.view = View::Help;
            }
            _ => {}
        }
    }

    fn on_key_dep_map(&mut self, key: crossterm::event::KeyEvent) {
        use KeyCode::*;
        match key.code {
            Esc | Char('m') => {
                self.view = self.previous_view.clone();
            }
            Up | Char('k') if self.dep_scroll > 0 => {
                self.dep_scroll -= 1;
            }
            Down | Char('j') => {
                self.dep_scroll += 1;
            }
            Char('x') => {
                // Clear all selections and go back to list.
                self.selected_queues.clear();
                self.selected_topics.clear();
                self.view = self.previous_view.clone();
            }
            _ => {}
        }
    }

    fn on_key_picker(&mut self, key: crossterm::event::KeyEvent) {
        use KeyCode::*;
        let is_profile = self.view == View::ProfilePicker;
        let list_len = if is_profile {
            self.profiles.len()
        } else {
            AWS_REGIONS.len()
        };
        match key.code {
            Up | Char('k') if self.picker_cursor > 0 => {
                self.picker_cursor -= 1;
            }
            Down | Char('j') if self.picker_cursor + 1 < list_len => {
                self.picker_cursor += 1;
            }
            Enter => {
                if is_profile {
                    self.profile_idx = self.picker_cursor;
                } else {
                    self.region_idx = self.picker_cursor;
                }
                persist::save_state(self.current_profile(), self.current_region());
                self.list_cursor = 0;
                self.view = self.previous_view.clone();
                self.schedule_refresh();
            }
            Esc => {
                self.view = self.previous_view.clone();
            }
            _ => {}
        }
    }

    fn on_key_help(&mut self, key: crossterm::event::KeyEvent) {
        use KeyCode::*;
        match key.code {
            Esc | Char('?') | Enter => {
                self.view = self.previous_view.clone();
            }
            _ => {}
        }
    }

    fn on_key_detail(&mut self, key: crossterm::event::KeyEvent) {
        use KeyCode::*;
        match key.code {
            Esc => {
                if self.view == View::SqsDetail {
                    self.active_sqs_queue_url = None;
                    self.queue_insights = None;
                    self.queue_cloudwatch_metrics = None;
                }
                self.view = match self.view {
                    View::SqsDetail => View::SqsList,
                    View::SnsDetail => View::SnsList,
                    _ => View::SqsList,
                };
                self.detail_scroll = 0;
                self.sub_scroll = 0;
                self.detail_on_subs = false;
            }
            // Tab switches focus between the two panels (SQS and SNS detail).
            Tab => {
                if matches!(self.view, View::SqsDetail | View::SnsDetail) {
                    self.detail_on_subs = !self.detail_on_subs;
                }
            }
            Up | Char('k') => {
                if self.detail_on_subs {
                    if self.sub_scroll > 0 {
                        self.sub_scroll -= 1;
                    }
                } else if self.detail_scroll > 0 {
                    self.detail_scroll -= 1;
                }
            }
            Down | Char('j') => {
                if self.detail_on_subs {
                    self.sub_scroll += 1;
                } else {
                    self.detail_scroll += 1;
                }
            }
            Char('?') => {
                self.previous_view = self.view.clone();
                self.view = View::Help;
            }
            _ => {}
        }
    }

    fn on_key_quit_confirm(&mut self, key: crossterm::event::KeyEvent) {
        use KeyCode::*;
        match key.code {
            Enter | Char('y') | Char('Y') => {
                self.should_quit = true;
            }
            Esc | Char('n') | Char('N') => {
                self.cancel_quit_confirm();
            }
            _ => {}
        }
    }

    // -------------------------------------------------------------------
    // Event responses
    // -------------------------------------------------------------------

    pub fn on_tick(&mut self) {
        if self.loading {
            self.loading_tick = self.loading_tick.wrapping_add(1);
        }

        // Debounced config-change refresh.
        if self.pending_refresh {
            if self.debounce_ticks > 0 {
                self.debounce_ticks -= 1;
            } else {
                self.pending_refresh = false;
                self.tick_counter = 0; // reset periodic timer to avoid back-to-back refreshes
                self.trigger_refresh();
                return;
            }
        }

        // Periodic auto-refresh.
        self.tick_counter += 1;
        if self.tick_counter >= AUTO_REFRESH_TICKS {
            self.tick_counter = 0;
            self.trigger_refresh();
        }
    }

    /// Schedules a refresh after a short debounce. Resets the countdown on
    /// repeated calls (e.g. rapid profile/region cycling).
    fn schedule_refresh(&mut self) {
        self.pending_refresh = true;
        self.debounce_ticks = DEBOUNCE_TICKS;
    }

    fn start_requests(&mut self, count: usize) {
        self.pending_requests += count;
        self.loading = self.pending_requests > 0;
    }

    fn finish_request(&mut self) {
        self.pending_requests = self.pending_requests.saturating_sub(1);
        self.loading = self.pending_requests > 0;
    }

    fn set_status(&mut self, level: StatusLevel, text: impl Into<String>) {
        self.status = Some(StatusMessage {
            level,
            text: text.into(),
        });
    }

    fn open_quit_confirm(&mut self) {
        if self.view != View::QuitConfirm {
            self.quit_return_view = Some(self.view.clone());
            self.view = View::QuitConfirm;
        }
    }

    fn cancel_quit_confirm(&mut self) {
        self.view = self.quit_return_view.take().unwrap_or(View::SqsList);
    }

    fn clamp_cursor_for_view(&mut self, view: View) {
        let len = match view {
            View::SqsList => self.filtered_queues().len(),
            View::SnsList => self.filtered_topics().len(),
            _ => 0,
        };

        self.list_cursor = if len == 0 {
            0
        } else {
            self.list_cursor.min(len - 1)
        };
    }

    fn refresh_queue_insights(&mut self) {
        let Some(detail) = self.queue_detail.as_ref() else {
            self.queue_insights = Some(QueueInsightsState::Loading);
            return;
        };

        self.queue_insights = match self.queue_cloudwatch_metrics.as_ref() {
            Some(Ok(metrics)) => Some(QueueInsightsState::Ready(Box::new(compute_queue_insights(
                detail,
                Some(metrics),
            )))),
            Some(Err(_)) => Some(QueueInsightsState::Ready(Box::new(compute_queue_insights(
                detail, None,
            )))),
            None => Some(QueueInsightsState::Loading),
        };
    }

    fn trigger_manual_refresh(&mut self) {
        match self.view {
            View::SqsDetail => self.refresh_active_sqs_detail(),
            _ => self.trigger_refresh(),
        }
    }

    fn refresh_active_sqs_detail(&mut self) {
        let Some(queue_url) = self.active_sqs_queue_url.clone() else {
            return;
        };

        self.status = None;
        self.queue_detail = None;
        self.queue_insights = Some(QueueInsightsState::Loading);
        self.queue_cloudwatch_metrics = None;
        self.start_requests(2);
        self.spawn_sqs_detail_requests(queue_url);
    }

    fn spawn_sqs_detail_requests(&self, queue_url: String) {
        let tx = self.event_tx.clone();
        let profile = self.current_profile().to_string();
        let region = self.current_region().to_string();
        let detail_url = queue_url.clone();
        tokio::spawn(async move {
            match load_sdk_config(&profile, &region).await {
                Ok(cfg) => {
                    let svc = SqsService::new(&cfg);
                    match svc.get_queue_detail(&detail_url).await {
                        Ok(detail) => {
                            let _ = tx.send(AppEvent::SqsDetailLoaded {
                                queue_url: detail_url,
                                detail,
                            });
                        }
                        Err(e) => {
                            let _ = tx.send(AppEvent::Error(e.to_string()));
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(AppEvent::Error(e.to_string()));
                }
            }
        });

        let tx = self.event_tx.clone();
        let profile = self.current_profile().to_string();
        let region = self.current_region().to_string();
        tokio::spawn(async move {
            match load_sdk_config(&profile, &region).await {
                Ok(cfg) => {
                    let svc = CloudWatchService::new(&cfg);
                    let result = svc
                        .get_sqs_queue_metrics(&queue_url)
                        .await
                        .map_err(|e| e.to_string());
                    let _ = tx.send(AppEvent::SqsCloudWatchLoaded { queue_url, result });
                }
                Err(e) => {
                    let _ = tx.send(AppEvent::SqsCloudWatchLoaded {
                        queue_url,
                        result: Err(e.to_string()),
                    });
                }
            }
        });
    }

    fn open_detail(&mut self) {
        match self.view {
            View::SqsList => {
                let queues = self.filtered_queues();
                let Some(queue) = queues.get(self.list_cursor) else {
                    return;
                };
                let url = queue.url.clone();
                self.view = View::SqsDetail;
                self.detail_scroll = 0;
                self.sub_scroll = 0;
                self.detail_on_subs = false;
                self.status = None;
                self.active_sqs_queue_url = Some(url.clone());
                self.queue_detail = None;
                self.queue_insights = Some(QueueInsightsState::Loading);
                self.queue_cloudwatch_metrics = None;
                self.start_requests(2);
                self.spawn_sqs_detail_requests(url);
            }
            View::SnsList => {
                let topics = self.filtered_topics();
                let Some(topic) = topics.get(self.list_cursor) else {
                    return;
                };
                let arn = topic.arn.clone();
                self.view = View::SnsDetail;
                self.detail_scroll = 0;
                self.sub_scroll = 0;
                self.detail_on_subs = false;
                self.active_sqs_queue_url = None;
                self.queue_insights = None;
                self.queue_cloudwatch_metrics = None;
                self.topic_detail = None;
                self.start_requests(1);
                let tx = self.event_tx.clone();
                let profile = self.current_profile().to_string();
                let region = self.current_region().to_string();
                tokio::spawn(async move {
                    match load_sdk_config(&profile, &region).await {
                        Ok(cfg) => {
                            let svc = SnsService::new(&cfg);
                            match svc.get_topic_detail(&arn).await {
                                Ok(detail) => {
                                    let _ = tx.send(AppEvent::SnsDetailLoaded(detail));
                                }
                                Err(e) => {
                                    let _ = tx.send(AppEvent::Error(e.to_string()));
                                }
                            }
                        }
                        Err(e) => {
                            let _ = tx.send(AppEvent::Error(e.to_string()));
                        }
                    }
                });
            }
            _ => {}
        }
    }

    /// Spawns async tasks to reload SQS queues and SNS topics.
    pub fn trigger_refresh(&mut self) {
        self.status = None;
        self.start_requests(3);
        let tx = self.event_tx.clone();
        let profile = self.current_profile().to_string();
        let region = self.current_region().to_string();

        // SQS
        {
            let tx = tx.clone();
            let profile = profile.clone();
            let region = region.clone();
            tokio::spawn(async move {
                match load_sdk_config(&profile, &region).await {
                    Ok(cfg) => {
                        let svc = SqsService::new(&cfg);
                        match svc.list_queues().await {
                            Ok(queues) => {
                                let _ = tx.send(AppEvent::SqsLoaded(queues));
                            }
                            Err(e) => {
                                let _ = tx.send(AppEvent::Error(format!("SQS: {e}")));
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(AppEvent::Error(format!("Config: {e}")));
                    }
                }
            });
        }

        // SNS topics
        {
            let tx = tx.clone();
            let profile = profile.clone();
            let region = region.clone();
            tokio::spawn(async move {
                match load_sdk_config(&profile, &region).await {
                    Ok(cfg) => {
                        let svc = SnsService::new(&cfg);
                        match svc.list_topics().await {
                            Ok(topics) => {
                                let _ = tx.send(AppEvent::SnsLoaded(topics));
                            }
                            Err(e) => {
                                let _ = tx.send(AppEvent::Error(format!("SNS: {e}")));
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(AppEvent::Error(format!("Config: {e}")));
                    }
                }
            });
        }

        // SNS → SQS subscription map
        tokio::spawn(async move {
            match load_sdk_config(&profile, &region).await {
                Ok(cfg) => {
                    let svc = SnsService::new(&cfg);
                    match svc.list_sqs_subscriptions().await {
                        Ok(map) => {
                            let _ = tx.send(AppEvent::SqsSnsMapLoaded(map));
                        }
                        Err(e) => {
                            let _ = tx.send(AppEvent::Error(format!("SQS/SNS map: {e}")));
                        }
                    }
                }
                Err(e) => {
                    let _ = tx.send(AppEvent::Error(format!("Config: {e}")));
                }
            }
        });
    }
}

// -------------------------------------------------------------------
// Main run loop
// -------------------------------------------------------------------

pub async fn run(terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>) -> Result<()> {
    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<AppEvent>();
    let mut input_rx = start_event_handler(Duration::from_millis(TICK_RATE_MS));

    let profiles = crate::aws::config::list_profiles();
    let mut app = App::new(profiles, event_tx);

    // Kick off initial data load.
    app.trigger_refresh();

    loop {
        // Draw
        terminal.draw(|f| ui::render(f, &app))?;

        // Merge events from both channels (input + async results).
        let event = tokio::select! {
            Some(e) = input_rx.recv() => e,
            Some(e) = event_rx.recv() => e,
        };

        match event {
            AppEvent::Key(key) => {
                use KeyCode::*;
                // Ctrl+C always quits immediately.
                let force_quit =
                    key.code == Char('c') && key.modifiers.contains(KeyModifiers::CONTROL);
                if force_quit {
                    break;
                }
                app.on_key(key);
                if app.should_quit {
                    break;
                }
            }
            AppEvent::Tick => {
                app.on_tick();
            }
            AppEvent::SqsLoaded(queues) => {
                app.queues = queues;
                if matches!(app.view, View::SqsList | View::SqsDetail) {
                    app.clamp_cursor_for_view(View::SqsList);
                }
                app.finish_request();
            }
            AppEvent::SnsLoaded(topics) => {
                app.topics = topics;
                if matches!(app.view, View::SnsList | View::SnsDetail) {
                    app.clamp_cursor_for_view(View::SnsList);
                }
                app.finish_request();
            }
            AppEvent::SqsDetailLoaded { queue_url, detail } => {
                if app.active_sqs_queue_url.as_deref() == Some(queue_url.as_str()) {
                    app.queue_detail = Some(detail);
                    app.refresh_queue_insights();
                }
                app.finish_request();
            }
            AppEvent::SqsCloudWatchLoaded { queue_url, result } => {
                if app.active_sqs_queue_url.as_deref() == Some(queue_url.as_str()) {
                    let warning = result.as_ref().err().cloned();
                    app.queue_cloudwatch_metrics = Some(result);
                    app.refresh_queue_insights();
                    if let Some(message) = warning {
                        app.set_status(
                            StatusLevel::Error,
                            format!("SQS insights unavailable: {message}"),
                        );
                    }
                }
                app.finish_request();
            }
            AppEvent::SnsDetailLoaded(detail) => {
                app.topic_detail = Some(detail);
                app.finish_request();
            }
            AppEvent::SqsSnsMapLoaded(map) => {
                app.sqs_sns_map = map;
                app.finish_request();
            }
            AppEvent::Error(msg) => {
                app.finish_request();
                app.set_status(StatusLevel::Error, msg);
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{InsightSeverity, QueueInsight, QueueInsights};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn app_for_test() -> App {
        let (tx, _rx) = mpsc::unbounded_channel();
        App::new(vec!["default".to_string()], tx)
    }

    fn ready_queue_insights() -> QueueInsightsState {
        let insight = QueueInsight {
            state: "ok".to_string(),
            detail: "ok".to_string(),
            severity: InsightSeverity::Normal,
        };

        QueueInsightsState::Ready(Box::new(QueueInsights {
            drain_outlook: insight.clone(),
            time_to_empty: insight.clone(),
            completion_pressure: insight.clone(),
            oldest_message_risk: insight.clone(),
            processing_pressure: insight,
        }))
    }

    #[test]
    fn command_r_is_manual_refresh_shortcut() {
        assert!(is_manual_refresh_key(KeyEvent::new(
            KeyCode::Char('r'),
            KeyModifiers::SUPER,
        )));
        assert!(is_manual_refresh_key(KeyEvent::new(
            KeyCode::Char('R'),
            KeyModifiers::SUPER | KeyModifiers::SHIFT,
        )));
    }

    #[test]
    fn f5_is_not_manual_refresh_shortcut() {
        assert!(!is_manual_refresh_key(KeyEvent::new(
            KeyCode::F(5),
            KeyModifiers::NONE,
        )));
    }

    #[test]
    fn plain_r_is_not_manual_refresh_shortcut() {
        assert!(!is_manual_refresh_key(KeyEvent::new(
            KeyCode::Char('r'),
            KeyModifiers::NONE,
        )));
    }

    #[test]
    fn command_r_with_extra_modifiers_is_not_manual_refresh_shortcut() {
        assert!(!is_manual_refresh_key(KeyEvent::new(
            KeyCode::Char('r'),
            KeyModifiers::SUPER | KeyModifiers::CONTROL,
        )));
    }

    #[tokio::test]
    async fn command_r_in_sqs_list_starts_global_refresh_requests() {
        let mut app = app_for_test();
        app.view = View::SqsList;
        app.status = Some(StatusMessage {
            level: StatusLevel::Info,
            text: "stale".to_string(),
        });

        app.on_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::SUPER));

        assert_eq!(app.pending_requests, 3);
        assert!(app.status.is_none());
    }

    #[tokio::test]
    async fn command_r_in_sqs_detail_refreshes_only_active_detail() {
        let mut app = app_for_test();
        let queue_url = "https://sqs.eu-west-1.amazonaws.com/123456789012/test-queue".to_string();
        app.view = View::SqsDetail;
        app.active_sqs_queue_url = Some(queue_url.clone());
        app.queue_detail = Some(QueueDetail {
            name: "test-queue".to_string(),
            arn: "arn:aws:sqs:eu-west-1:123456789012:test-queue".to_string(),
            attributes: vec![("VisibilityTimeout".to_string(), "30".to_string())],
        });
        app.queue_insights = Some(ready_queue_insights());
        app.queue_cloudwatch_metrics = Some(Ok(QueueCloudWatchMetrics::default()));
        app.detail_scroll = 4;
        app.sub_scroll = 2;
        app.detail_on_subs = true;
        app.status = Some(StatusMessage {
            level: StatusLevel::Error,
            text: "old error".to_string(),
        });

        app.on_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::SUPER));

        assert_eq!(app.view, View::SqsDetail);
        assert_eq!(
            app.active_sqs_queue_url.as_deref(),
            Some(queue_url.as_str())
        );
        assert!(app.queue_detail.is_none());
        assert!(matches!(
            app.queue_insights,
            Some(QueueInsightsState::Loading)
        ));
        assert!(app.queue_cloudwatch_metrics.is_none());
        assert_eq!(app.pending_requests, 2);
        assert_eq!(app.detail_scroll, 4);
        assert_eq!(app.sub_scroll, 2);
        assert!(app.detail_on_subs);
        assert!(app.status.is_none());
    }

    #[tokio::test]
    async fn command_r_in_sqs_detail_without_active_url_is_noop() {
        let mut app = app_for_test();
        app.view = View::SqsDetail;
        app.queue_detail = Some(QueueDetail {
            name: "test-queue".to_string(),
            arn: "arn:aws:sqs:eu-west-1:123456789012:test-queue".to_string(),
            attributes: vec![("VisibilityTimeout".to_string(), "30".to_string())],
        });
        app.queue_insights = Some(ready_queue_insights());

        app.on_key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::SUPER));

        assert!(app.queue_detail.is_some());
        assert!(matches!(
            app.queue_insights,
            Some(QueueInsightsState::Ready(_))
        ));
        assert_eq!(app.pending_requests, 0);
    }

    #[test]
    fn esc_clears_applied_search_filter_in_list_view() {
        let mut app = app_for_test();
        app.search_query = "*-dlq".to_string();
        app.view = View::SqsList;

        app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        assert!(app.search_query.is_empty());
        assert_eq!(app.list_cursor, 0);
    }

    #[test]
    fn q_opens_quit_confirm_from_list_view() {
        let mut app = app_for_test();
        app.view = View::SqsList;

        app.on_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));

        assert_eq!(app.view, View::QuitConfirm);
        assert_eq!(app.quit_return_view, Some(View::SqsList));
        assert!(!app.should_quit);
    }

    #[test]
    fn q_in_search_mode_is_literal_input_not_quit_modal() {
        let mut app = app_for_test();
        app.view = View::SqsList;
        app.search_active = true;

        app.on_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));

        assert_eq!(app.view, View::SqsList);
        assert_eq!(app.search_query, "q");
    }

    #[test]
    fn search_mode_accepts_star_literal_input() {
        let mut app = app_for_test();
        app.view = View::SqsList;
        app.search_active = true;

        app.on_key(KeyEvent::new(KeyCode::Char('*'), KeyModifiers::NONE));

        assert_eq!(app.search_query, "*");
        assert_eq!(app.list_cursor, 0);
    }

    #[test]
    fn search_mode_accepts_question_mark_literal_input() {
        let mut app = app_for_test();
        app.view = View::SqsList;
        app.search_active = true;

        app.on_key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));

        assert_eq!(app.search_query, "?");
        assert_eq!(app.list_cursor, 0);
    }

    #[test]
    fn canceling_quit_confirm_restores_originating_view() {
        let mut app = app_for_test();
        app.view = View::ProfilePicker;
        app.previous_view = View::SnsList;

        app.on_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));
        app.on_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        assert_eq!(app.view, View::ProfilePicker);
        assert_eq!(app.previous_view, View::SnsList);
        assert_eq!(app.quit_return_view, None);
        assert!(!app.should_quit);
    }

    #[test]
    fn confirming_quit_sets_should_quit() {
        let mut app = app_for_test();
        app.view = View::SqsDetail;

        app.on_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));
        app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

        assert_eq!(app.view, View::QuitConfirm);
        assert!(app.should_quit);
    }
}
