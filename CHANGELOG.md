# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-04-02

### Added
- TUI browser for AWS SNS topics and SQS queues
- SQS queue list with message count indicators (total, invisible, delayed)
- SQS queue detail view with full attributes and SNS subscriptions
- SNS topic list with subscription counts
- SNS topic detail view with full attributes and subscription list
- Dependency map view: ASCII tree of SNS topic ↔ SQS queue edges with filter policies
- Profile and region picker (reads `~/.aws/config` and `~/.aws/credentials`)
- Search/filter in list views
- Sort by name, message count ascending/descending
- Multi-select for dependency map filtering
- Clipboard export (markdown) of current view
- Persistent profile + region state in `~/.local/share/bridgio/state`
- Auto-refresh every ~30 seconds
- Help overlay with all keybindings
