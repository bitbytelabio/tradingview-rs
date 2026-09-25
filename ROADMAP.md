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

## v0.3.0 — Data Coverage (completed)

> **Goal:** Expand the data surface to cover fundamental data, economic calendar, and scanner APIs.

| Status | Task | Priority |
|--------|------|----------|
| ✅ | Fundamental data retrieval (one-shot `fetch_full_fundamentals`, `StudyClient`, registry, CSV export) | 🔴 |
| ✅ | Economic calendar events (`get_economic_calendar`, earnings, dividends, macro releases) | 🔴 |
| ✅ | `FinancialPeriod` deserialization hardening (handle edge cases from TV) | 🟡 |
| ✅ | Technical analysis scanner ratings across 8 timeframes (`get_technical_analysis`) | 🟡 |
| ✅ | UTF-16 code units length-prefixed packet parsing for Unicode/CJK symbols | 🔴 |
| ✅ | Publish v0.3.0 to crates.io | 🔴 |

## v0.4.0 — Python Bindings, Authentication & Transport (completed)

> **Goal:** High-performance PyO3 Python bindings, user session authentication, and automated CAPTCHA solving.

| Status | Task | Priority |
|--------|------|----------|
| ✅ | Native Python bindings (`tradingview-py` / `tradingview-rs` on PyPI) with PyO3 0.29 and stable ABI (`abi3`) | 🔴 |
| ✅ | Direct Polars & Pandas DataFrame integration for historical OHLCV, fundamentals, and economic calendar | 🔴 |
| ✅ | Real-time dual-mode streaming: async iterators (`async for`) and synchronous callbacks | 🔴 |
| ✅ | Configurable WebSocket data server endpoint selection (`DataServer::ProData`, `DataServer::Data`, etc.) | 🟡 |
| ✅ | Modular `user` authentication architecture (`src/user/{mod.rs, captcha.rs}`) | 🔴 |
| ✅ | Standard RFC 6238 Base32 & whitespace-grouped TOTP 2FA parsing (`totp-rs`) and `otpauth://` URI support | 🟡 |
| ✅ | Automated reCAPTCHA v2 solving via 2Captcha with strict single-task budget and `reportIncorrect` refund reporting | 🟡 |
| ✅ | Python API parity for 2Captcha solver via `captcha_key` parameter on `login()` and `authenticate()` | 🟡 |
| ✅ | Private Unix `0o600` cookie file export with overwrite protection in `examples/user.rs` | 🟢 |
| ✅ | Hardened multi-platform CI/CD: automated crates.io trusted publishing and PyPI binary wheel distribution | 🔴 |

## v0.5.0 — Screener, Performance & Observability (current)

> **Goal:** Symbol screener, production-grade telemetry, metrics, and memory optimization.

| Status | Task | Priority |
|--------|------|----------|
| ⬜ | Symbol screener integration (filter by sector, market cap, P/E, volume, etc.) | 🔴 |
| ⬜ | Screener streaming & polling events into `MarketEvent::Screener` | 🟡 |
| ⬜ | Normalize all data types into typed `MarketEvent` variants | 🟡 |
| ⬜ | Zero-copy deserialization for `SocketMessageDe` (avoiding intermediate `serde_json::Value`) | 🟡 |
| ⬜ | `tracing` spans on every major operation (source fetch, sink accept, fan-out) | 🟡 |
| ⬜ | Prometheus metrics exporter: message throughput, latency percentiles, error rate, channel depth | 🟡 |
| ⬜ | `CommandRunner` connection statistics exposed as public API (`ConnectionStats`, `CommandQueueStats`) | 🟢 |
| ⬜ | WebSocket connection pooling for concurrent multi-symbol subscriptions | 🟢 |
| ⬜ | Benchmark suite: `criterion` benches for `HistoricalClient`, packet framing, and fan-out throughput | 🟡 |
| ⬜ | Fuzz testing for packet parser and `SocketMessage` deserialization | 🟢 |

## v1.0.0 — Stable API

> **Goal:** API stabilization, security hardening, and production readiness.

| Task | Priority |
|------|----------|
| Freeze public API surface — all `pub` items audited for naming, consistency, and discoverability | 🔴 |
| Semver compliance from v1.0.0 forward across Rust and Python packages | 🔴 |
| Remove deprecated types, aliases, and experimental feature flags | 🔴 |
| Comprehensive integration test suite covering real TradingView sessions | 🔴 |
| `SECURITY.md` with vulnerability reporting + dependency audit in CI (`cargo audit`) | 🟡 |
| Automated performance regression testing in CI | 🟡 |
| `CONTRIBUTING.md` with local dev setup, test conventions, PR template | 🟢 |
| Full migration guide from v0.x → v1.0 in CHANGELOG.md | 🟡 |
| Official release announcement + documentation site | ⚪ |

## Backlog (Unscheduled)

Features under consideration but not yet assigned to a milestone:

- Public chat interactions (TradingView community chat rooms)
- `HistoricalClient` pluggable storage backends (Parquet, DuckDB, ClickHouse)
- `DataLoader` hot-reload: add/remove sinks dynamically at runtime
- WASM compilation target for browser-based client usage
- Replay mode enhancements (variable speed, step-through, seek-to-timestamp)
- Technical analysis signal computation engine (client-side RSI, MACD, Bollinger Bands)
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
