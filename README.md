# TradingView Data Source

![Tests](https://github.com/bitbytelabio/tradingview-rs/actions/workflows/ci.yml/badge.svg)
![GitHub latest commit](https://img.shields.io/github/last-commit/bitbytelabio/tradingView-rs)
[![Crates.io](https://img.shields.io/crates/v/tradingview-rs)](https://crates.io/crates/tradingview-rs)
[![Documentation](https://docs.rs/tradingview-rs/badge.svg)](https://docs.rs/tradingview-rs)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

## Introduction

This is a data source library for algorithmic trading written in Rust inspired by [TradingView-API](https://github.com/Mathieu2301/TradingView-API). It provides programmatic access to TradingView's data and features through a robust, async-first API.

⚠️ **Alpha Stage**: This library is currently in **alpha** stage and not ready for production use. Breaking changes may occur between versions.

## Features

- [x] **Async Support** - Built with Tokio for high-performance async operations
- [x] **Multi-Threading** - Handle large amounts of data efficiently
- [x] **Session Management** - Shared sessions between threads to respect TradingView's rate limits
- [x] **TradingView Premium Features** - Access premium data and indicators
- [x] **Real-time Data** - WebSocket-based live market data
- [x] **Historical Data** - Fetch OHLCV data with batch operations
- [x] **Custom Indicators** - Work with Pine Script indicators
- [x] **Chart Drawings** - Retrieve your chart drawings and annotations
- [x] **Replay Mode** - Historical market replay functionality
- [x] **Symbol Search** - Search and filter symbols by market, country, and type
- [x] **News Integration** - Access TradingView news and headlines
- [ ] Fundamental data
- [ ] Technical analysis signals
- [ ] Invite-only indicators support
- [ ] Public chat interactions
- [ ] Screener integration
- [ ] Economic calendar
- [ ] Vectorized data conversion

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
tradingview-rs = { git = "https://github.com/bitbytelabio/tradingview-rs.git", branch = "main" }
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

## Examples

The [`examples/`](examples/) directory contains comprehensive examples:

- [`historical_data.rs`](examples/historical_data.rs) - Fetch historical OHLCV data for a single symbol
- [`historical_data_batch.rs`](examples/historical_data_batch.rs) - Batch historical data operations
- [`historical_data_with_replay.rs`](examples/historical_data_with_replay.rs) - Historical data with replay mode
- [`live.rs`](examples/live.rs) - Real-time market data via WebSocket
- [`user.rs`](examples/user.rs) - User authentication and session management
- [`indicator.rs`](examples/indicator.rs) - Working with Pine Script indicators
- [`search.rs`](examples/search.rs) - Symbol search and filtering
- [`misc.rs`](examples/misc.rs) - Miscellaneous utility functions

Run an example:

```bash
cargo run --example historical_data
cargo run --example live
cargo run --example search
```

## Prerequisites

- **Rust 1.70+** - This library uses modern Rust features
- **TradingView Account** - Required for authenticated features
- **Network Access** - Connects to TradingView's servers

### Environment Variables

For examples requiring authentication, create a `.env` file:

```env
TV_USERNAME=your_username
TV_PASSWORD=your_password
TV_TOTP_SECRET=your_2fa_secret  # Optional, for 2FA
TV_AUTH_TOKEN=your_auth_token   # Get from user authentication
```

### Feature Flags

Some examples require specific features to be enabled:

```toml
[dependencies]
tradingview-rs = { git = "https://github.com/bitbytelabio/tradingview-rs.git", branch = "main", features = ["user"] }
```

## Use Cases

- **[VNQuant Datafeed](https://github.com/bitbytelabio/vnquant-datafeed)** - Event-driven data engine with RedPanda (Kafka)
- **Algorithmic Trading Bots** - Real-time market data for trading strategies
- **Market Research** - Historical data analysis and backtesting
- **Portfolio Management** - Track and analyze investment performance
- **Technical Analysis** - Custom indicators and studies

## Documentation

Since this library is in **alpha stage**, documentation is actively being developed. The best way to learn is through the examples in the [`examples/`](examples/) directory.

## Before Opening an Issue

1. **Check existing issues** - Your problem might already be reported
2. **Update to latest version** - Bug fixes are released regularly
3. **Review examples** - Make sure you're using the API correctly
4. **Provide minimal reproduction** - Include code that demonstrates the issue
5. **Include error messages** - Full error output helps with debugging

## Known Issues & Limitations

- **Rate Limiting** - TradingView enforces rate limits; respect them to avoid bans
- **Session Expiry** - User sessions expire and need renewal
- **Alpha Quality** - Breaking changes may occur between versions
- **Premium Features** - Some features require TradingView Pro/Premium subscription
- **Indicator Data Loading** - Some study data series loading needs fixes (see TODO in indicator example)

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
