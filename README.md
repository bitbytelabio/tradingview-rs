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

### Basic Historical Data

```rust
use tradingview::{
    Interval, OHLCV,
    chart::{ChartOptions, fetch_chart_data},
    socket::DataServer,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get this token from `get_quote_token(cookies)` function
    let auth_token = std::env::var("TV_AUTH_TOKEN").expect("TV_AUTH_TOKEN is not set");

    let option =ChartOptions::builder()
        .symbol("AAPL")
        .exchange("NASDAQ")
        .interval(Interval::OneDay);

    let data = fetch_chart_data(&auth_token, option, None).await?;

    Ok(())
}
```

### User Authentication

```rust
use tradingview::UserCookies;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let user = UserCookies::default()
        .login("username", "password", Some("totp_secret"))
        .await?;

    // Use authenticated user for premium features
    Ok(())
}
```

### Real-time Data

```rust
use tradingview::{
    Interval, QuoteValue,
    callback::EventCallback,
    chart::ChartOptions,
    pine_indicator::ScriptType,
    socket::DataServer,
    websocket::{WebSocket, WebSocketClient},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let auth_token = env::var("TV_AUTH_TOKEN").expect("TV_AUTH_TOKEN is not set");

    let quote_callback = |data: QuoteValue| {
        println!("{:#?}", data);
    };

    let callbacks: EventCallback = EventCallback::default().on_quote_data(quote_callback);
    let client = WebSocketClient::default().set_callbacks(callbacks);
    let websocket = WebSocket::new()
        .server(DataServer::ProData)
        .auth_token(&auth_token)
        .client(client)
        .build()
        .await
        .unwrap();

    websocket
        .create_quote_session()
        .await?
        .set_fields()
        .await?
        .add_symbols(vec![
            "SP:SPX",
            "BINANCE:BTCUSDT",
            "BINANCE:ETHUSDT",
            "BITSTAMP:ETHUSD",
            "NASDAQ:TSLA",
        ])
        .await?;

    websocket.subscribe().await

    Ok(())
}
```

## Examples

The [`examples/`](examples/) directory contains comprehensive examples:

- [`historical_data.rs`](examples/historical_data.rs) - Fetch historical OHLCV data
- [`fetch_historical_data_batch.rs`](examples/fetch_historical_data_batch.rs) - Batch historical data operations
- [`user.rs`](examples/user.rs) - User authentication and session management
- [`indicator.rs`](examples/indicator.rs) - Working with Pine Script indicators
- [`misc.rs`](examples/misc.rs) - Miscellaneous utility functions

Run an example:

```bash
cargo run --example historical_data
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
```

## Use Cases

- **[VNQuant Datafeed](https://github.com/bitbytelabio/vnquant-datafeed)** - Event-driven data engine with RedPanda (Kafka)
- **Algorithmic Trading Bots** - Real-time market data for trading strategies
- **Market Research** - Historical data analysis and backtesting
- **Portfolio Management** - Track and analyze investment performance

## Documentation

Since this library is in **alpha stage**, documentation is actively being developed.

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
