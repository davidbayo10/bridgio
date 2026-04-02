# AGENTS.md

Instructions for AI coding agents working on **bridgio**.

## Project Overview

Rust TUI application (single binary, no CLI arguments) for browsing AWS SNS topics and SQS queues. Purely interactive — all configuration happens through the UI.

## Architecture

```
src/
├── main.rs          Terminal setup (raw mode, alternate screen), event loop bootstrap
├── app.rs           Central App struct (state machine) + handle_key_event()
├── models.rs        Data types: View, QueueInfo, TopicInfo, SortMode, SqsSnsSubscription
├── context.rs       Builds markdown from current view state for clipboard export
├── event.rs         AppEvent enum + spawns async event handler (input + AWS responses)
├── error.rs         AppError enum wrapping AWS and IO errors
├── clipboard.rs     Platform-specific clipboard (pbcopy / clip / xclip / xsel / wl-copy)
├── persist.rs       Saves/loads profile + region to ~/.local/share/bridgio/state
├── aws/
│   ├── mod.rs       Re-exports
│   ├── config.rs    AWS profile discovery from ~/.aws/config + credentials, SDK config loading
│   ├── sqs.rs       list_queues() (paginated + concurrent attributes), get_queue_detail()
│   └── sns.rs       list_topics(), get_topic_detail(), list_sqs_subscriptions() (SNS→SQS map)
└── ui/
    ├── mod.rs       Top-level render() dispatch based on View enum
    ├── header.rs    Profile/region display + tab bar (1:SQS | 2:SNS | ?:Help)
    ├── help.rs      Keybindings popup overlay
    ├── picker.rs    Modal selector for profile/region (centered Clear + bordered Block)
    ├── sqs_list.rs  SQS queue table with colored message counts
    ├── sqs_detail.rs  Split panel: queue attributes + SNS subscriptions
    ├── sns_list.rs  SNS topic table with subscription counts
    ├── sns_detail.rs  Split panel: topic attributes + subscriptions
    └── dep_map.rs   ASCII tree of selected topic↔queue edges with filter policies
```

## Key Types

| Type | Location | Purpose |
|------|----------|---------|
| `App` | app.rs | Central state: view, data, UI cursors, selections, search, loading |
| `View` | models.rs | Navigation state machine (SqsList, SqsDetail, SnsList, SnsDetail, DependencyMap, Help, ProfilePicker, RegionPicker) |
| `AppEvent` | event.rs | Async channel messages (Key, Tick, SqsLoaded, SnsLoaded, SqsDetailLoaded, SnsDetailLoaded, SqsSnsMapLoaded, Error) |
| `QueueInfo` | models.rs | Queue summary: name, ARN, URL, message counts |
| `QueueDetail` | models.rs | Full queue attributes |
| `TopicInfo` | models.rs | Topic summary: name, ARN, subscription count |
| `TopicDetail` | models.rs | Topic attributes + subscription list |
| `SqsSnsSubscription` | models.rs | SNS→SQS edge with topic ARN, subscription ARN, filter policy |
| `SortMode` | models.rs | Name, MessagesDesc, MessagesAsc |
| `AppError` | error.rs | Wraps AWS SDK and IO errors |

## Patterns

### Event Loop

crossterm polls at 250ms tick rate → events sent to tokio mpsc channel → `App` dispatches via `AppEvent` match. Auto-refresh every 120 ticks (~30s). Profile/region changes debounce ~1s before triggering refresh.

### AWS Concurrency

All AWS list operations are paginated. Attribute fetching uses `Arc<Semaphore>` with 20 permits to parallelize API calls without overwhelming rate limits. Tasks are spawned via `tokio::spawn` and results collected with `futures::future::join_all`.

### View Navigation

`View` enum drives which render function is called in `ui/mod.rs` and which key handlers are active in `app.rs`. `previous_view` enables `Esc` to go back.

### Error Handling

AWS/IO errors → `AppError` → sent as `AppEvent::Error(String)` through the channel → displayed in status bar with red text and ✗ prefix. Clipboard errors fail silently.

## UI Conventions

### Colors

| Color | Meaning |
|-------|---------|
| Cyan | Active panel borders, headers, key highlights, non-zero sub counts |
| Yellow | Loading indicator, attribute keys, sort mode, input prompts |
| Green | Selected items (✓), orphan queue markers |
| Red | High message counts (≥ 1000) |
| Light Yellow | Medium message counts (≥ 100) |
| Dark Gray | Inactive panel borders, ARNs, loading skeletons |
| White | Normal text |

### Layout

Header (3 rows) → Content area → Status bar. Modals use centered `Clear` widget with bordered `Block`.

## How to Extend

### Add a new view

1. Add variant to `View` enum in `models.rs`
2. Add render function in `ui/` (new file or existing)
3. Add dispatch arm in `ui/mod.rs` `render()`
4. Add key handling branch in `app.rs` `handle_key_event()`
5. Add navigation logic (set `view` / `previous_view`)

### Add a new AWS service

1. Create module in `src/aws/` with list/detail functions
2. Define data types in `models.rs`
3. Add `AppEvent` variants for loaded data in `event.rs`
4. Spawn async fetch in the event handler
5. Store results in `App` struct and add corresponding views

### Add a new keybinding

Add the `KeyCode` match arm in the appropriate view block inside `handle_key_event()` in `app.rs`.

## Build Commands

```bash
cargo build              # Debug build
cargo build --release    # Release build
cargo clippy             # Lint
cargo fmt                # Format
```
