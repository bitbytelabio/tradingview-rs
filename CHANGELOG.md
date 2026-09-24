# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed
- Linux OpenSSL/BoringSSL linker collision by enabling `prefix-symbols` on `wreq`.

## [0.4.1] - 2026-09-24

### Added
- Python `DataServer` enum (`Data`, `ProData`, `WidgetData`, `MobileData`) for configurable TradingView WebSocket data server endpoint selection.
- Keyword-only `server` parameter (`server=DataServer.Data`) in `TradingViewClient` constructor and `login()`, with a read-only `server` property.
- Optional keyword-only `server` parameter override in high-level client methods: `get_historical`, `get_historical_df`, `get_historical_batch`, `subscribe_quotes`, `subscribe_bars`, and `get_fundamental`.
- Async `get_tradingview_token()` method on Python `TradingViewClient` (and Rust `get_tradingview_token`) to retrieve a TradingView session token using authenticated session cookies.
- Offline rejection with `AuthenticationError` when `get_tradingview_token()` is called on anonymous or token-only clients before making network requests.
- Support for both RFC 6238 Base32 secrets and `otpauth://totp/...` URIs in `totp_secret` parameter for `login()` and `authenticate()`.
- Automatic initial `set_auth_token` dispatch in `WebSocketClient` prior to session command execution.

### Changed
- Preserved default `DataServer.Data` endpoint and backward-compatible positional API across all client methods.
- Migrated entire HTTP client transport from `reqwest` to `wreq` (v6.0) + `wreq-util` (v3.0) with browser emulation.
- Migrated 2FA TOTP implementation from `google-authenticator` to `totp-rs` (v6.0) with `std`, `otpauth`, and `zeroize` features.
- Renamed `get_quote_token` to `get_tradingview_token` across Rust and Python without aliases.
- Cargo TLS flags (`rustls-tls`, `native-tls`) now scope exclusively to WebSocket connections via `tokio-tungstenite`.
- Clarified `DataServer.ProData` access semantics: anonymous connection is supported for public data, while paid market data feeds require account and feed entitlements.
## [0.4.0] - 2026-09-22

### Added
- Native Python bindings (`tradingview-py`) built with PyO3 0.29, `abi3` stable ABI (Python >= 3.10), and Tokio async runtime integration.
- Direct Polars DataFrame conversion across historical OHLCV data, batch retrieval, corporate fundamentals, and economic calendar queries (`as_dataframe=True` and `get_historical_df`).
- Dual-mode real-time streaming: async iterators (`async for`) and synchronous callbacks with exception isolation (`sys.unraisablehook`) and thread-safe event loop trampolining.
- Python exception hierarchy (`TradingViewError`, `AuthenticationError`, `SymbolNotFoundError`, `ConnectionError`, `TimeoutError`, `RateLimitError`, `ProtocolError`) with orphan-rule-safe Rust error mapping.
- Core error enrichment: added typed `Error::RateLimited` variant and enriched HTTP 429 status code handling.
- CI/CD workflow for automated Python wheel building and PyPI publishing via Twine.
- Comprehensive documentation with Mermaid architecture diagrams in `README.md` and dedicated guides in `docs/`.

### Changed
- Converted project into a virtual Cargo workspace with `crates/tradingview` (core library) and `crates/tradingview-py` (Python extension).
- Relocated core library source, tests, examples, and benchmarks into `crates/tradingview/`.
## [0.3.0] - 2026-09-10

### Added
- One-argument full fundamental fetch API (`fetch_full_fundamentals("AAPL") -> FullFundamentalResult`) with ranked stock resolution, typed per-study outcomes, and optional CSV export
- High-level one-shot study retrieval client (`tradingview::study::{StudyClient, StudyRequest, StudyResult}`) modeled after `HistoricalClient`
- Global economic calendar API client (`tradingview::client::fin_calendar::{get_economic_calendar, EconomicCalendarRequest, EconomicCalendarEvent, EconomicImportance}`)
- Runnable example for full fundamental catalog retrieval and lossless long-form CSV export (`examples/full_fundamental_fetch.rs`)
- Technical Analysis scanner API (`get_technical_analysis`, `TechnicalAnalysis`, `Period`, `TechnicalAnalysisRecommendations`)
- Chart `Range` options support and export in historical series retrieval
- Candle data source metadata and symbol data-feed prefix field support

### Changed
- `Symbol::id()` now prefers non-empty `prefix` (e.g. `AMEX:SPY`), falling back to `exchange`
- Updated default User-Agent header to modern browser target

### Performance
- Zero-copy structural length-prefixed WebSocket packet parser replacing regex cleaner and splitter
- Zero-copy packet parser tests and UTF-16 framing support for Unicode scripts (Vietnamese HOSE, Taiwanese TWSE)
- `gen_id()` returns `String` without permanently interning into global `Ustr` hash map
- Replaced 100ms sleep polling in `HistoricalClient::retrieve` with `tokio::sync::Notify`

### Fixed
- Protocol wire payloads for `modify_study` (4 args) and `replay_*` commands (request ID, milliseconds interval, replay session)
- Heartbeat echo framing and TradingView protocol frame handling

## [0.2.0] - 2026-06-10

### Added
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
- Fan-out optimization in `DataLoader` moving events into final sink and cloning only for preceding active sinks

### Fixed
- 7 broken intra-doc links in module-level docs (now zero `cargo doc` warnings)
- Duplicate `EconomicCategory` definition in models/mod.rs

### Known Issues
- 7 `parse_packet` / `SocketMessage` round-trip test failures (pre-existing, tracked in v0.2.0)

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
