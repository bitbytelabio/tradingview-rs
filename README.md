# TradingView Data Source

![Tests](https://github.com/bitbytelabio/tradingview-rs/actions/workflows/ci.yml/badge.svg)
![GitHub latest commit](https://img.shields.io/github/last-commit/bitbytelabio/tradingView-rs)
[![Crates.io](https://img.shields.io/crates/v/tradingview-rs)](https://crates.io/crates/tradingview-rs)
[![Documentation](https://docs.rs/tradingview-rs/badge.svg)](https://docs.rs/tradingview-rs)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

## Introduction

This is a data source library for algorithmic trading written in Rust inspired by [TradingView-API](https://github.com/Mathieu2301/TradingView-API). It provides programmatic access to TradingView's data and features through a robust, async-first API.

The library exposes **two usage tiers**:
- **High-level** — An event-driven [`DataLoader`](https://docs.rs/tradingview-rs/latest/tradingview/loader/struct.DataLoader.html) that connects a source to multiple sinks with backpressure and graceful shutdown.
- **Low-level** — Direct access to HTTP clients, WebSocket sessions, and raw message parsing for full control.

**Status**: `tradingview-rs` is a stable community data source library under active development. Note that it is an unofficial integration subject to upstream TradingView changes; review version notes and test against your workload before deploying to production.

## Features

- [x] **Async Support** — Built with Tokio for high-performance async operations
- [x] **Event-Driven Pipeline** — `DataSource` → fan-out → `EventSink` architecture with bounded channels, cancellation tokens, and error recovery
- [x] **Multiple Sinks** — Built-in channel, callback, and Kafka (RedPanda) sinks; implement your own via the `EventSink` trait
- [x] **Real-time Data** — WebSocket-based live market data with automatic reconnection and circuit breaker
- [x] **Historical Data** — Fetch OHLCV data for single symbols and concurrent batch operations
- [x] **Session Management** — Shared sessions between threads to respect TradingView's rate limits
- [x] **Custom Indicators** — Work with Pine Script indicators via study configurations
- [x] **Chart Drawings** — Retrieve your chart drawings and annotations
- [x] **Replay Mode** — Historical market replay functionality
- [x] **Symbol Search** — Search and filter symbols by market, country, and type
- [x] **News Integration** — Access TradingView news and headlines
- [x] **User Authentication** — Login with username/password + TOTP 2FA support
- [x] **Premium Features** — Access TradingView Pro/Premium/Expert data tiers
- [x] **Fundamental data** — Built-in Pine study catalog & date-versioned registry (`tradingview::fundamental`)
- [x] **Technical analysis signals** — Retrieve scanner ratings across 8 timeframes (via get_technical_analysis)
- [x] **Invite-only indicators support** — Access private Pine Script indicators (via get_private_indicators)
- [ ] Public chat interactions
- [ ] Screener integration
- [x] **Economic calendar** — Global macroeconomic events endpoint (`tradingview::client::fin_calendar`)
- [ ] Vectorized data conversion

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
# From crates.io (recommended):
tradingview-rs = "0.3"

# Or from the Git repository:
tradingview-rs = { git = "https://github.com/bitbytelabio/tradingview-rs.git", branch = "main" }
```

### Feature Flags

| Feature | Default | Description |
|---------|---------|-------------|
| `rustls-tls` | ✅ | TLS via rustls (recommended) |
| `native-tls` | — | TLS via platform-native libraries |
| `user` | ✅ | User authentication (login, 2FA, session cookies) |

Example with optional features:

```toml
[dependencies]
tradingview-rs = { version = "0.3", default-features = false, features = ["native-tls", "user"] }
```

## Quick Start

### Historical Data (Single Symbol)

```rust
use tradingview::{DataServer, Interval, history};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let auth_token = std::env::var("TV_AUTH_TOKEN").expect("TV_AUTH_TOKEN is not set");

    let (_info, data) = history::single::retrieve()
        .auth_token(&auth_token)
        .symbol("BTCUSDT")
        .exchange("BINANCE")
        .interval(Interval::OneHour)
        .with_replay(true)
        .server(DataServer::ProData)
        .call()
        .await?;

    println!("Retrieved {} data points", data.len());
    Ok(())
}
```

### Historical Data (Batch)

```rust
use tradingview::{Interval, Symbol, history};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let auth_token = std::env::var("TV_AUTH_TOKEN").expect("TV_AUTH_TOKEN is not set");

    let symbols = vec![
        Symbol::builder().symbol("BTCUSDT").exchange("BINANCE").build(),
        Symbol::builder().symbol("ETHUSDT").exchange("BINANCE").build(),
    ];

    let datamap = history::batch::retrieve()
        .auth_token(&auth_token)
        .symbols(&symbols)
        .interval(Interval::OneHour)
        .call()
        .await?;

    for (symbol_info, ticker_data) in datamap.values() {
        println!("{}: {} data points", symbol_info.name, ticker_data.len());
    }

    Ok(())
}
```

### Symbol Search

```rust
use tradingview::{list_symbols, prelude::*};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let symbols = list_symbols()
        .market_type(MarketType::All)
        .call()
        .await?;

    println!("Found {} symbols", symbols.len());
    Ok(())
}
```

### User Authentication

```rust
use tradingview::UserCookies;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let username = std::env::var("TV_USERNAME").expect("TV_USERNAME is not set");
    let password = std::env::var("TV_PASSWORD").expect("TV_PASSWORD is not set");
    let totp = std::env::var("TV_TOTP_SECRET").expect("TV_TOTP_SECRET is not set");

    let user = UserCookies::default()
        .login(&username, &password, Some(&totp))
        .await?;

    // Save cookies for later use
    let json = serde_json::to_string_pretty(&user)?;
    std::fs::write("tv_user_cookies.json", json)?;

    Ok(())
}
```

### Real-time Data

```rust
use dotenv::dotenv;
use std::{env, sync::Arc, time::Duration};
use tokio::{sync::mpsc, time::sleep};
use tradingview::{
    ChartOptions, Interval,
    live::{
        handler::{
            command::CommandRunner,
            message::{Command, TradingViewResponse},
        },
        models::DataServer,
        websocket::WebSocketClient,
    },
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();

    let auth_token = env::var("TV_AUTH_TOKEN").expect("TV_AUTH_TOKEN is not set");

    // Create communication channels
    let (response_tx, mut response_rx) = mpsc::unbounded_channel();
    let (command_tx, command_rx) = mpsc::unbounded_channel();

    // Create WebSocket client
    let ws_client = WebSocketClient::builder()
        .auth_token(&auth_token)
        .server(DataServer::ProData)
        .data_tx(response_tx)
        .build()
        .await?;

    // Create command runner
    let command_runner = CommandRunner::new(command_rx, Arc::clone(&ws_client));

    // Spawn command runner
    tokio::spawn(async move {
        command_runner.run().await.unwrap();
    });

    // Handle responses
    tokio::spawn(async move {
        while let Some(response) = response_rx.recv().await {
            match response {
                TradingViewResponse::ChartData(series_info, data_points) => {
                    println!("Chart Data: {} points", data_points.len());
                }
                TradingViewResponse::QuoteData(quote) => {
                    println!("Quote: {:?}", quote);
                }
                _ => {}
            }
        }
    });

    // Set up market data
    let options = ChartOptions::builder()
        .symbol("BTCUSDT".into())
        .exchange("BINANCE".into())
        .interval(Interval::OneMinute)
        .build();

    command_tx.send(Command::set_market(options))?;
    command_tx.send(Command::add_symbol("NASDAQ:AAPL"))?;

    // Keep running
    sleep(Duration::from_secs(60)).await;

    Ok(())
}
```

### Working with Indicators

```rust
use tradingview::{
    ChartOptions, Interval, StudyOptions,
    get_builtin_indicators,
    pine_indicator::{BuiltinIndicators, ScriptType},
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Get built-in indicators
    let indicators = get_builtin_indicators(BuiltinIndicators::Standard).await?;

    if let Some(indicator) = indicators.first() {
        let opts = ChartOptions::builder()
            .symbol("BTCUSDT".into())
            .exchange("BINANCE".into())
            .interval(Interval::OneDay)
            .bar_count(20)
            .study_config(StudyOptions {
                script_id: (&indicator.script_id).into(),
                script_version: (&indicator.script_version).into(),
                script_type: ScriptType::IntervalScript,
            })
            .build();

        // Use opts with WebSocket client for real-time indicator data
    }

    Ok(())
}
```

### Full Fundamental Data With One Stock Code

```rust
use tradingview::fundamental::{FullFundamentalResult, fetch_full_fundamentals};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Bare tickers are resolved through TradingView's ranked stock search.
    // Canonical values such as "HOSE:FPT" or "TWSE:2330" are also accepted.
    let result: FullFundamentalResult = fetch_full_fundamentals("AAPL").await?;

    println!("{}: {} studies", result.canonical_id(), result.total_studies());
    println!(
        "success={}, empty={}, errors={}",
        result.success_count(),
        result.empty_count(),
        result.error_count()
    );

    // Optional lossless long-form CSV export from the returned struct.
    result.write_csv("AAPL_fundamentals.csv")?;
    Ok(())
}
```

The returned `FullFundamentalResult` owns the resolved stock metadata, registry date/schema
version, and every metric result as `Success(Vec<DataPoint>)`, `Empty`, or `Error(String)`.
The full stock catalog excludes crypto-only `STD;CryptoFund_*` studies.

Run the example with one stock code:

```bash
cargo run --example full_fundamental_fetch -- AAPL
cargo run --example full_fundamental_fetch -- HOSE:FPT
cargo run --example full_fundamental_fetch -- TWSE:2330
```

### Economic Calendar

```rust
use chrono::{Duration, Utc};
use tradingview::client::fin_calendar::{
    EconomicCalendarRequest, EconomicImportance, get_economic_calendar,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let now = Utc::now();
    let request = EconomicCalendarRequest::builder()
        .from(now)
        .to(now + Duration::days(7))
        .countries(vec!["US".to_string(), "DE".to_string()])
        .min_importance(EconomicImportance::Medium)
        .build();

    let events = get_economic_calendar(&request).await?;
    for event in events {
        println!("{}: {} (importance: {:?})", event.date, event.title, event.importance_level());
    }
    Ok(())
}
```

## Examples

The [`examples/`](examples/) directory contains runnable examples for every major feature:

| Example | Description |
|---------|-------------|
| [`historical_data_fetch.rs`](examples/historical_data_fetch.rs) | Fetch historical OHLCV for a single symbol |
| [`batch_historical_fetch.rs`](examples/batch_historical_fetch.rs) | Concurrent batch historical data |
| [`live_quote.rs`](examples/live_quote.rs) | Real-time quote streaming via WebSocket |
| [`channel_consumer.rs`](examples/channel_consumer.rs) | Event-driven loader with a channel sink |
| [`callback_consumer.rs`](examples/callback_consumer.rs) | Event-driven loader with an inline callback sink |
| [`user.rs`](examples/user.rs) | User authentication and session management |
| [`search.rs`](examples/search.rs) | Symbol search and filtering |
| [`misc.rs`](examples/misc.rs) | Miscellaneous utility functions |
| [`full_fundamental_fetch.rs`](examples/full_fundamental_fetch.rs) | Fetch full fundamental Pine studies catalog from one stock code and export to CSV |

Run an example:

```bash
cargo run --example historical_data_fetch
cargo run --example live_quote
cargo run --example channel_consumer
cargo run --example full_fundamental_fetch -- AAPL
```

## Prerequisites

- **Rust 1.85+** (edition 2024) — This library uses modern Rust features
- **TradingView Account** — Required for authenticated features (free tier works for most)
- **Network Access** — Connects to TradingView's servers

### Environment Variables

For examples requiring authentication, create a `.env` file:

```env
TV_USERNAME=your_username
TV_PASSWORD=your_password
TV_TOTP_SECRET=your_2fa_secret  # Optional, for 2FA
TV_AUTH_TOKEN=your_auth_token   # Get from user authentication
```

## Architecture

```text
┌──────────────────────────────────────────────────┐
│                   tradingview-rs                   │
├──────────────────────────────────────────────────┤
│  High-Level API (event-driven)                    │
│  ┌──────────┐    ┌───────────┐    ┌────────────┐ │
│  │  Source   │───▶│ DataLoader │───▶│ EventSink  │ │
│  │ (TV feed) │    │  (fan-out) │    │ (channel,  │ │
│  │           │    │            │    │  callback, │ │
│  │           │    │            │    │  kafka)    │ │
│  └──────────┘    └───────────┘    └────────────┘ │
├──────────────────────────────────────────────────┤
│  Low-Level API (direct access)                    │
│  ┌──────────────┐ ┌──────────────┐ ┌───────────┐ │
│  │  historical  │ │    live      │ │   client   │ │
│  │ (WebSocket)  │ │ (WebSocket)  │ │  (REST)    │ │
│  └──────────────┘ └──────────────┘ └───────────┘ │
├──────────────────────────────────────────────────┤
│  Shared: models, chart, quote, error, utils       │
└──────────────────────────────────────────────────┘
```

| Module | Purpose |
|--------|---------|
| [`historical`](https://docs.rs/tradingview-rs/latest/tradingview/historical/) | Single + batch OHLCV retrieval via WebSocket |
| [`live`](https://docs.rs/tradingview-rs/latest/tradingview/live/) | Real-time WebSocket streaming (quotes, charts, studies) |
| [`client`](https://docs.rs/tradingview-rs/latest/tradingview/client/) | REST HTTP client (search, news, financial calendar) |
| [`loader`](https://docs.rs/tradingview-rs/latest/tradingview/loader/) | Event-driven orchestrator (source → fan-out → sinks) |
| [`source`](https://docs.rs/tradingview-rs/latest/tradingview/source/) | `DataSource` trait + TradingView WebSocket adapter |
| [`sink`](https://docs.rs/tradingview-rs/latest/tradingview/sink/) | `EventSink` trait + channel, callback, Kafka sinks |
| [`events`](https://docs.rs/tradingview-rs/latest/tradingview/events/) | Normalized `MarketEvent` types (Candle, Quote, News, etc.) |
| [`chart`](https://docs.rs/tradingview-rs/latest/tradingview/chart/) | Chart session config + Pine Script studies |
| [`quote`](https://docs.rs/tradingview-rs/latest/tradingview/quote/) | Real-time quote data model + field definitions |

## Use Cases

- **[VNQuant Datafeed](https://github.com/bitbytelabio/vnquant-datafeed)** — Event-driven data engine with RedPanda (Kafka)
- **Algorithmic Trading Bots** — Real-time market data for trading strategies
- **Market Research** — Historical data analysis and backtesting
- **Portfolio Management** — Track and analyze investment performance
- **Technical Analysis** — Custom indicators and studies

## Documentation

Full API documentation is published on [docs.rs](https://docs.rs/tradingview-rs). All public types, traits, and modules are documented with examples.

Quick links to key types:
- [`DataLoader`](https://docs.rs/tradingview-rs/latest/tradingview/loader/struct.DataLoader.html) — event-driven orchestrator
- [`HistoricalClient`](https://docs.rs/tradingview-rs/latest/tradingview/historical/client/struct.HistoricalClient.html) — historical data
- [`WebSocketClient`](https://docs.rs/tradingview-rs/latest/tradingview/websocket/struct.WebSocketClient.html) — real-time streaming
- [`Symbol`](https://docs.rs/tradingview-rs/latest/tradingview/models/struct.Symbol.html) — instrument representation
- [`Interval`](https://docs.rs/tradingview-rs/latest/tradingview/models/enum.Interval.html) — time granularity

For the project roadmap, see [ROADMAP.md](ROADMAP.md).

## Before Opening an Issue

1. **Check existing issues** - Your problem might already be reported
2. **Update to latest version** - Bug fixes are released regularly
3. **Review examples** - Make sure you're using the API correctly
4. **Provide minimal reproduction** - Include code that demonstrates the issue
5. **Include error messages** - Full error output helps with debugging

## Known Issues & Limitations

- **Rate Limiting** — TradingView enforces rate limits; respect them to avoid bans
- **Session Expiry** — User sessions expire periodically and need renewal
- **API Stability** — Breaking changes may occur across minor releases prior to 1.0; consult CHANGELOG.md when updating.
- **Premium Features** — Some features require TradingView Pro/Premium/Expert subscription
- **Study Series Loading** — Some Pine Script study data series need fixes (see `TODO` in indicator code)

## Roadmap

See [ROADMAP.md](ROADMAP.md) for planned features, milestones, and version timeline.

## Contributing

Contributions are welcome! Please read our [Code of Conduct](CODE_OF_CONDUCT.md) first.

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

## Security

If you discover a security vulnerability, please see our [Security Policy](SECURITY.md) for reporting instructions.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Disclaimer

This library is not affiliated with TradingView. Use at your own risk and ensure compliance with TradingView's Terms of Service.
