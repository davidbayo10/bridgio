# bridgio

[![CI](https://github.com/avantio/bridgio/actions/workflows/ci.yml/badge.svg)](https://github.com/avantio/bridgio/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/bridgio.svg)](https://crates.io/crates/bridgio)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Terminal UI for exploring AWS SNS topics and SQS queues.

Built with Rust, ratatui, and the AWS SDK — browse messaging infrastructure across multiple profiles and regions from your terminal.

## Install

```bash
cargo install bridgio
```

Or build from source:

```bash
cargo build --release
./target/release/bridgio
```



## Features

- Browse SQS queues with message counts (visible, in-flight, delayed)
- Browse SNS topics with subscription counts
- Detailed attribute views for individual queues and topics
- SNS → SQS subscription exploration with filter policies
- Visual dependency map showing topic-to-queue edges (ASCII tree)
- Multi-profile and multi-region support (16 AWS regions)
- Search and filter by name, sort by name or message count
- Multi-select resources for dependency analysis
- Copy context to clipboard in markdown format
- Profile and region persistence across sessions

## Requirements

- Rust 2024 edition
- AWS credentials configured in `~/.aws/config` and/or `~/.aws/credentials`

## Build & Run

```bash
cargo build --release
./target/release/bridgio
```

## Keybindings

### Global

| Key | Action |
|-----|--------|
| `q` / `Ctrl+C` | Quit |
| `1` | Switch to SQS list |
| `2` | Switch to SNS list |
| `?` | Toggle help |
| `p` / `P` | Open profile picker |
| `r` / `R` | Open region picker |
| `F5` | Refresh |
| `c` | Copy context to clipboard |

### List views

| Key | Action |
|-----|--------|
| `↑` / `k` | Cursor up |
| `↓` / `j` | Cursor down |
| `Enter` | Open detail view |
| `/` | Start search (filter by name) |
| `s` | Cycle sort: Name → Messages ↓ → Messages ↑ (SQS only) |
| `Space` | Toggle selection |
| `m` | Open dependency map (requires selections) |
| `x` | Clear all selections |

### Detail views

| Key | Action |
|-----|--------|
| `Tab` | Switch focus between panels |
| `↑` / `k` | Scroll up |
| `↓` / `j` | Scroll down |
| `Esc` | Back to list |

### Search mode

| Key | Action |
|-----|--------|
| Any character | Append to query |
| `Backspace` | Delete last character |
| `Esc` / `Enter` | Exit search |

## Configuration

**Profile discovery** — Reads profiles from `~/.aws/config` and `~/.aws/credentials`. The `default` profile is always available.

**Regions** — 16 hardcoded AWS regions. Default: `eu-west-1`.

**Persistence** — Last used profile and region are saved to `~/.local/share/bridgio/state` and restored on next launch.

## Tech Stack

| Crate | Purpose |
|-------|---------|
| [ratatui](https://crates.io/crates/ratatui) 0.30 | TUI rendering framework |
| [crossterm](https://crates.io/crates/crossterm) 0.29 | Terminal control |
| [tokio](https://crates.io/crates/tokio) 1.50 | Async runtime |
| [aws-sdk-sqs](https://crates.io/crates/aws-sdk-sqs) 1.97 | SQS client |
| [aws-sdk-sns](https://crates.io/crates/aws-sdk-sns) 1.98 | SNS client |
| [aws-config](https://crates.io/crates/aws-config) 1.8 | AWS SDK configuration |
| [anyhow](https://crates.io/crates/anyhow) 1.0 | Error handling |
