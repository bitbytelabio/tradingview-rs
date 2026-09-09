# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- One-argument full fundamental fetch API (`fetch_full_fundamentals("AAPL") -> FullFundamentalResult`) with ranked stock resolution, typed per-study outcomes, and optional CSV export
- High-level one-shot study retrieval client (`tradingview::study::{StudyClient, StudyRequest, StudyResult}`) modeled after `HistoricalClient`
- Global economic calendar API client (`tradingview::client::fin_calendar::{get_economic_calendar, EconomicCalendarRequest, EconomicCalendarEvent, EconomicImportance}`)
- Runnable example for full fundamental catalog retrieval and lossless long-form CSV export (`examples/full_fundamental_fetch.rs`)
- Technical Analysis scanner API (`get_technical_analysis`, `TechnicalAnalysis`, `Period`, `TechnicalAnalysisRecommendations`)
- Event-driven `DataLoader` with source → fan-out → sinks architecture
- `DataSource` trait with TradingView WebSocket adapter (`source::tradingview`)
- `EventSink` trait with three built-in sinks: `ChannelSink`, `CallbackSink`, `KafkaSink`
- `MarketEvent` enum (Candle, Quote, Economic, News variants)
- `Subscription` and `LoaderConfig` for data pipeline configuration
- `BoundedChannel` with configurable capacity in all sink paths
- Channel-based write path in `WebSocketClient` (eliminates `Arc<Mutex<SplitSink>>` lock contention)
- Circuit breaker pattern in `WebSocketClient` for error recovery
- `ConnectionHealth` and `HealthMetrics` monitoring in WebSocket client
- `ErrorSeverity` classification for WebSocket errors
- Full API documentation (`///` doc comments) on all public types
- Module-level docs for `client`, `live`, `chart`, `quote`, `models`
- ROADMAP.md with version milestones
- CONTRIBUTING.md with developer guidelines
- GitHub project setup script (`scripts/gh_setup.sh`)

### Changed
- `Symbol::id()` now prefers non-empty `prefix` (e.g. `AMEX:SPY`), falling back to `exchange`
- README: added architecture diagram, module table, feature table, roadmap link
- README: updated Installation section with crates.io + feature flag table
- README: updated Examples section with all current examples
- `models/mod.rs`: documented all ~30 public types (`Interval`, `Symbol`, `MarketType`, etc.)
- `error.rs`: documented `Error`, `TradingViewError`, `LoginError` variants
- `chart/options.rs`: documented `ChartOptions`, `Range`, `StudyOptions`
- `live/websocket.rs`: documented `WebSocketClient`, `ErrorSeverity`, `ConnectionHealth`, `SeriesInfo`
- `live/models.rs`: documented `TradingViewDataEvent`, `SocketMessageSer`, `SocketMessageDe`, `SocketServerInfo`, `DataServer`
- `live/handler/command.rs`: documented `CommandPriority`, `Command`, `ConnectionStatus`, `CommandRunner`

### Performance
- Zero-copy structural length-prefixed WebSocket packet parser replacing regex cleaner and splitter
- `gen_id()` returns `String` without permanently interning into global `Ustr` hash map
- Replaced 100ms sleep polling in `HistoricalClient::retrieve` with `tokio::sync::Notify`
- Fan-out optimization in `DataLoader` moving events into final sink and cloning only for preceding active sinks

### Fixed
- Protocol wire payloads for `modify_study` (4 args) and `replay_*` commands (request ID, milliseconds interval, replay session)
- 7 broken intra-doc links in module-level docs (now zero `cargo doc` warnings)
- Duplicate `EconomicCategory` definition in models/mod.rs

## [0.1.2] — 2025

Initial alpha release with core data retrieval capabilities.

### Features
- Historical OHLCV data (single + batch)
- Real-time WebSocket streaming (quotes, charts, studies)
- Symbol search with market type filtering
- User authentication with TOTP 2FA support
- Pine Script built-in indicator catalog
- Chart drawings retrieval
- Replay mode for historical market data
- News feed integration
