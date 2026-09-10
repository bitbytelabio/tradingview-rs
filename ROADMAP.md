# Roadmap

This document outlines the planned development trajectory for `tradingview-rs`. Milestones are organized by version and tagged with priority (🔴 critical, 🟡 high, 🟢 medium, ⚪ low).

## v0.2.0 — Pipeline & Polish

> **Goal:** Ship the event-driven pipeline, improve developer experience, and harden existing features.

| Status | Task | Priority |
|--------|------|----------|
| ✅ | Event-driven `DataLoader` with source → fan-out → sinks architecture | 🔴 |
| ✅ | `DataSource` trait + TradingView WebSocket adapter | 🔴 |
| ✅ | `EventSink` trait + built-in sinks (channel, callback, Kafka) | 🔴 |
| ✅ | `MarketEvent` enum (Candle, Quote, Economic, News variants) | 🔴 |
| ✅ | Full API documentation (`cargo doc`) with zero warnings | 🟡 |
| ✅ | Module-level docs for all public modules | 🟡 |
| ✅ | README overhaul with architecture diagram and module table | 🟡 |
| ✅ | ROADMAP.md (this file) | 🟡 |
| ✅ | Bounded channel backpressure support in sink and loader channels | 🟡 |
| ⬜ | Graceful shutdown with in-flight batch draining | 🟡 |
| ⬜ | Rate-limit-aware request throttling in `HistoricalClient` | 🟡 |
| ✅ | CI: add `cargo doc` lint to CI pipeline (deny warnings) | 🟢 |
| ✅ | Publish v0.2.0 to crates.io | 🔴 |

## v0.3.0 — Data Coverage (current)

> **Goal:** Expand the data surface to cover fundamental data, economic calendar, and scanner APIs.

| Status | Task | Priority |
|--------|------|----------|
| ✅ | Fundamental data retrieval (one-shot `fetch_full_fundamentals`, `StudyClient`, registry, CSV export) | 🔴 |
| ✅ | Economic calendar events (`get_economic_calendar`, earnings, dividends, macro releases) | 🔴 |
| ⬜ | Symbol screener integration (filter by sector, market cap, P/E, etc.) | 🟡 |
| ✅ | `FinancialPeriod` deserialization hardening (handle edge cases from TV) | 🟡 |
| ⬜ | Normalize all data types into `MarketEvent` variants | 🟡 |
| ⬜ | Add `FundamentalEvent`, `EconomicEvent`, `ScreenerEvent` to `MarketEvent` | 🟡 |
| ⬜ | Pagination support for symbol search results | 🟢 |
| ⬜ | `MarketStatus` endpoint (holiday calendar per exchange) | 🟢 |

## v0.4.0 — Advanced Features

> **Goal:** Technical analysis signals, invite-only indicators, and advanced workflows.

| Status | Task | Priority |
|--------|------|----------|
| ✅ | Technical analysis signals (`get_technical_analysis` scanner ratings across 8 timeframes) | 🟡 |
| ✅ | Invite-only / private Pine Script indicator support (`get_private_indicators`) | 🟡 |
| ⬜ | Technical analysis signal computation (RSI, MACD, Bollinger bands, moving averages on client side) | 🟡 |
| ⬜ | Multi-timeframe analysis (concurrent subscriptions across intervals) | 🟡 |
| ⬜ | `StudyConfiguration` builder ergonomics — validate inputs before sending to TV | 🟢 |
| ⬜ | Indicator catalog caching (`get_builtin_indicators`) with TTL | 🟢 |
| ⬜ | Streaming indicator values as `MarketEvent::Indicator` | 🟡 |
| ⬜ | WebSocket connection pooling for multi-symbol subscriptions | 🟢 |
| ⬜ | `ChartDrawing` CRUD — create, update, delete drawings via API | ⚪ |

## v0.5.0 — Performance & Observability

> **Goal:** Production-grade performance, metrics, and debugging tools.

| Status | Task | Priority |
|--------|------|----------|
| ✅ | Zero-copy length-prefixed packet parsing for WebSocket messages | 🟡 |
| ⬜ | Zero-copy deserialization for `SocketMessageDe` (avoid `serde_json::Value` where possible) | 🟡 |
| ⬜ | `tracing` spans on every major operation (source fetch, sink accept, fan-out) | 🟡 |
| ⬜ | Prometheus metrics: message count, latency percentiles, error rate, channel depth | 🟡 |
| ⬜ | `CommandRunner` stats exposed as public API (`ConnectionStats`, `CommandQueueStats`) | 🟢 |
| ⬜ | Circuit breaker metrics (open/closed state, failure count, last trip time) | 🟢 |
| ⬜ | Benchmark suite: `criterion` benches for `HistoricalClient`, `SocketMessage` parse, fan-out throughput | 🟡 |
| ⬜ | Fuzz testing for `SocketMessage` deserialization | 🟢 |
| ⬜ | `DataLoader` integration tests with mock source/sink | 🟡 |

## v1.0.0 — Stable API

> **Goal:** API stabilization, security hardening, and production readiness.

| Task | Priority |
|------|----------|
| Freeze public API surface — all `pub` items audited for naming, consistency, and discoverability | 🔴 |
| Semver compliance from v1.0.0 forward | 🔴 |
| Remove deprecated types and aliases | 🔴 |
| Comprehensive integration test suite covering real TradingView sessions (CI with secrets) | 🔴 |
| `SECURITY.md` with vulnerability reporting + dependency audit in CI (`cargo audit`) | 🟡 |
| Performance regression testing in CI | 🟡 |
| `CONTRIBUTING.md` with local dev setup, test conventions, PR template | 🟢 |
| CHANGELOG.md with full migration guide from v0.x → v1.0 | 🟡 |
| Official release announcement + migration guide blog post | ⚪ |

## Backlog (Unscheduled)

Features under consideration but not yet assigned to a milestone:

- Public chat interactions (TradingView chat rooms)
- Vectorized data conversion (Polars/Arrow output from OHLCV)
- `HistoricalClient` pluggable storage backends (CSV, Parquet, Postgres)
- `DataLoader` hot-reload: add/remove sinks at runtime
- WASM compilation target for browser-based usage
- Python bindings via PyO3
- Replay mode enhancements (variable speed, step-through, seek-to-timestamp)

---

## Milestone Workflow

Each milestone corresponds to a GitHub Milestone. Issues are tagged with the milestone and one of these labels:

| Label | Meaning |
|-------|---------|
| `bug` | Something is broken |
| `enhancement` | New feature or improvement |
| `documentation` | Docs, README, examples, doc comments |
| `performance` | Optimization work |
| `testing` | Tests, fuzzing, benchmarks |
| `ci/cd` | CI pipeline, releases, automation |
| `breaking` | Breaking API change (v1.0.0 gate) |

## How to Contribute

Pick an unclaimed issue from the current milestone, comment on it, and open a PR referencing the issue. See [CONTRIBUTING.md](CONTRIBUTING.md) for detailed guidelines.
