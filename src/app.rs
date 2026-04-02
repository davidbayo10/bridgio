use std::collections::{HashMap, HashSet};
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tokio::sync::mpsc;

use crate::aws::config::load_sdk_config;
use crate::aws::sns::SnsService;
use crate::aws::sqs::SqsService;
use crate::event::{AppEvent, start_event_handler};
use crate::models::{
    AWS_REGIONS, QueueDetail, QueueInfo, SortMode, SqsSnsSubscription, TopicDetail, TopicInfo, View,
};
use crate::persist;
use crate::ui;

/// How often the event loop ticks when there is no input (ms).
const TICK_RATE_MS: u64 = 250;

/// How many ticks between automatic background refreshes (~30 s).
const AUTO_REFRESH_TICKS: u64 = 30_000 / TICK_RATE_MS;

/// Ticks to wait after the last config change before firing a refresh (~1 s).
const DEBOUNCE_TICKS: u64 = 4;

/// Central application state.
pub struct App {
    // Navigation
    pub view: View,
    pub previous_view: View,

    // AWS connection settings
    pub profiles: Vec<String>,
    pub profile_idx: usize,
    pub region_idx: usize,

    // Data
    pub queues: Vec<QueueInfo>,
    pub topics: Vec<TopicInfo>,
    pub queue_detail: Option<QueueDetail>,
    pub topic_detail: Option<TopicDetail>,
    /// SNS→SQS subscriptions, keyed by queue ARN.
    pub sqs_sns_map: HashMap<String, Vec<SqsSnsSubscription>>,

    // UI state
    pub list_cursor: usize,
    pub detail_scroll: usize,
    pub loading: bool,
    pub status: Option<String>,

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

    // Internal
    tick_counter: u64,
    pending_refresh: bool,
    debounce_ticks: u64,
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
            profiles,
            profile_idx,
            region_idx,
            queues: Vec::new(),
            topics: Vec::new(),
            queue_detail: None,
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
            tick_counter: 0,
            pending_refresh: false,
            debounce_ticks: 0,
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
        let q = self.search_query.to_lowercase();
        let mut result: Vec<QueueInfo> = self
            .queues
            .iter()
            .filter(|queue| q.is_empty() || queue.name.to_lowercase().contains(&q))
            .cloned()
            .collect();
        match self.sort_mode {
            SortMode::Name => {} // already sorted by name from AWS layer
            SortMode::MessagesDesc => {
                result.sort_by(|a, b| b.approx_messages.cmp(&a.approx_messages));
            }
            SortMode::MessagesAsc => {
                result.sort_by_key(|q| q.approx_messages);
            }
        }
        result
    }

    /// Topics matching `search_query` (always name-sorted).
    pub fn filtered_topics(&self) -> Vec<TopicInfo> {
        let q = self.search_query.to_lowercase();
        self.topics
            .iter()
            .filter(|t| q.is_empty() || t.name.to_lowercase().contains(&q))
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

        // 'c' copies context to clipboard from any view (except while typing in search).
        if key.code == Char('c') && !self.search_active {
            match crate::context::build(self) {
                Some(text) => {
                    if crate::clipboard::copy(&text) {
                        self.status = Some("✓ copied to clipboard".to_string());
                    } else {
                        self.status = Some("✗ clipboard unavailable".to_string());
                    }
                }
                None => {
                    self.status = Some("nothing to copy in this view".to_string());
                }
            }
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

        match &self.view {
            View::Help => {
                self.view = self.previous_view.clone();
            }
            View::ProfilePicker | View::RegionPicker => self.on_key_picker(key),
            View::SqsList | View::SnsList => self.on_key_list(key),
            View::SqsDetail | View::SnsDetail => self.on_key_detail(key),
            View::DependencyMap => self.on_key_dep_map(key),
        }
    }

    fn on_key_list(&mut self, key: crossterm::event::KeyEvent) {
        use KeyCode::*;
        match key.code {
            // View switch (resets search)
            Char('1') => {
                if self.view != View::SqsList {
                    self.view = View::SqsList;
                    self.list_cursor = 0;
                    self.search_query.clear();
                    self.search_active = false;
                }
            }
            Char('2') => {
                if self.view != View::SnsList {
                    self.view = View::SnsList;
                    self.list_cursor = 0;
                    self.search_query.clear();
                    self.search_active = false;
                }
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
            Up | Char('k') => {
                if self.list_cursor > 0 {
                    self.list_cursor -= 1;
                }
            }
            Down | Char('j') => {
                if self.list_cursor + 1 < self.list_len() {
                    self.list_cursor += 1;
                }
            }
            // Open detail
            Enter => self.open_detail(),
            // Refresh
            F(5) => self.trigger_refresh(),
            // Search
            Char('/') => {
                self.search_active = true;
            }
            // Sort (SQS only): Name → ↓msgs → ↑msgs → Name
            Char('s') => {
                if self.view == View::SqsList {
                    self.sort_mode = match self.sort_mode {
                        SortMode::Name => SortMode::MessagesDesc,
                        SortMode::MessagesDesc => SortMode::MessagesAsc,
                        SortMode::MessagesAsc => SortMode::Name,
                    };
                    self.list_cursor = 0;
                }
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
            Char('m') => {
                if !self.selected_queues.is_empty() || !self.selected_topics.is_empty() {
                    self.previous_view = self.view.clone();
                    self.dep_scroll = 0;
                    self.view = View::DependencyMap;
                }
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
            Esc | Char('q') | Char('m') => {
                self.view = self.previous_view.clone();
            }
            Up | Char('k') => {
                if self.dep_scroll > 0 {
                    self.dep_scroll -= 1;
                }
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
            Up | Char('k') => {
                if self.picker_cursor > 0 {
                    self.picker_cursor -= 1;
                }
            }
            Down | Char('j') => {
                if self.picker_cursor + 1 < list_len {
                    self.picker_cursor += 1;
                }
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
            Esc | Char('q') => {
                self.view = self.previous_view.clone();
            }
            _ => {}
        }
    }

    fn on_key_detail(&mut self, key: crossterm::event::KeyEvent) {
        use KeyCode::*;
        match key.code {
            Esc | Char('q') => {
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

    fn open_detail(&mut self) {
        match self.view {
            View::SqsList => {
                let queues = self.filtered_queues();
                if queues.is_empty() {
                    return;
                }
                let url = queues[self.list_cursor].url.clone();
                self.view = View::SqsDetail;
                self.detail_scroll = 0;
                self.loading = true;
                let tx = self.event_tx.clone();
                let profile = self.current_profile().to_string();
                let region = self.current_region().to_string();
                tokio::spawn(async move {
                    match load_sdk_config(&profile, &region).await {
                        Ok(cfg) => {
                            let svc = SqsService::new(&cfg);
                            match svc.get_queue_detail(&url).await {
                                Ok(detail) => {
                                    let _ = tx.send(AppEvent::SqsDetailLoaded(detail));
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
            View::SnsList => {
                let topics = self.filtered_topics();
                if topics.is_empty() {
                    return;
                }
                let arn = topics[self.list_cursor].arn.clone();
                self.view = View::SnsDetail;
                self.detail_scroll = 0;
                self.loading = true;
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
        self.loading = true;
        self.status = None;
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
                // Ctrl+C always quits; 'q' only quits outside search mode.
                let force_quit =
                    key.code == Char('c') && key.modifiers.contains(KeyModifiers::CONTROL);
                let soft_quit = !app.search_active && key.code == Char('q');
                if force_quit || soft_quit {
                    break;
                }
                app.on_key(key);
            }
            AppEvent::Tick => {
                app.on_tick();
            }
            AppEvent::SqsLoaded(queues) => {
                app.queues = queues;
                app.loading = false;
            }
            AppEvent::SnsLoaded(topics) => {
                app.topics = topics;
                app.loading = false;
            }
            AppEvent::SqsDetailLoaded(detail) => {
                app.queue_detail = Some(detail);
                app.loading = false;
            }
            AppEvent::SnsDetailLoaded(detail) => {
                app.topic_detail = Some(detail);
                app.loading = false;
            }
            AppEvent::SqsSnsMapLoaded(map) => {
                app.sqs_sns_map = map;
            }
            AppEvent::Error(msg) => {
                app.loading = false;
                app.status = Some(msg);
            }
        }
    }

    Ok(())
}
