//! Historical data fetch example.
//!
//! Demonstrates how to use `HistoricalClient` to retrieve OHLCV chart data
//! from TradingView.
//!
//! # Usage
//!
//! Set `TV_AUTH_TOKEN` in your environment or a `.env` file, then run:
//!
//! ```bash
//! cargo run --example historical_data_fetch
//! ```
//!
//! Or with specific symbol/exchange:
//!
//! ```bash
//! TV_SYMBOL=AAPL TV_EXCHANGE=NASDAQ cargo run --example historical_data_fetch
//! ```

use std::env;
use tradingview::{
    DataServer, Interval,
    historical::{HistoricalClient, HistoricalRequest},
    prelude::OHLCV,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing so we can see what's happening.
    tracing_subscriber::fmt::init();

    // ── Configuration ──────────────────────────────────────────────────
    let auth_token = env::var("TV_AUTH_TOKEN").unwrap_or_else(|_| {
        eprintln!("TV_AUTH_TOKEN not set — using anonymous access");
        "unauthorized_user_token".to_string()
    });

    let symbol = env::var("TV_SYMBOL").unwrap_or_else(|_| "AAPL".to_string());
    let exchange = env::var("TV_EXCHANGE").unwrap_or_else(|_| "NASDAQ".to_string());
    let server = env::var("TV_SERVER")
        .map(|s| match s.as_str() {
            "pro" => DataServer::ProData,
            "data" => DataServer::Data,
            _ => DataServer::ProData,
        })
        .unwrap_or(DataServer::ProData);

    // ── Build the request ─────────────────────────────────────────────
    let request = HistoricalRequest::builder()
        .symbol(symbol.clone())
        .exchange(exchange.clone())
        .interval(Interval::OneDay)
        .num_bars(100)
        .timeout(std::time::Duration::from_secs(30))
        .build();

    println!("Fetching {exchange}:{symbol} …");

    // ── Execute ────────────────────────────────────────────────────────
    let client = HistoricalClient::new(&auth_token, server);
    let result = client.retrieve(request).await?;

    // ── Display results ────────────────────────────────────────────────
    println!();
    println!("══ Historical Data Result ══");
    println!("  Symbol:        {}", result.symbol_info.name);
    println!("  Exchange:      {}", result.symbol_info.exchange);
    println!("  Description:   {}", result.symbol_info.description);
    println!("  Currency:      {}", result.symbol_info.currency_code);
    println!("  Market type:   {}", result.symbol_info.market_type);
    println!("  ─────────────────────────────");
    println!("  Bars received: {}", result.total_bars_received);
    println!("  Bars returned: {}", result.data.len());
    println!("  Replay used:   {}", result.replay_used);
    println!("  Elapsed:       {:?}", result.elapsed);
    println!();

    if let (Some(first), Some(last)) = (result.first_datetime(), result.last_datetime()) {
        println!("  First bar: {}", first.format("%Y-%m-%d %H:%M"));
        println!("  Last bar:  {}", last.format("%Y-%m-%d %H:%M"));
        println!();
    }

    // Print first 5 and last 5 bars.
    println!("══ First 5 bars ══");
    println!(
        "  {:>12} {:>10} {:>10} {:>10} {:>10} {:>12}",
        "Date", "Open", "High", "Low", "Close", "Volume"
    );
    for dp in result.data.iter().take(5) {
        let dt = dp.datetime();
        println!(
            "  {:>12} {:>10.2} {:>10.2} {:>10.2} {:>10.2} {:>12.0}",
            dt.format("%Y-%m-%d"),
            dp.open(),
            dp.high(),
            dp.low(),
            dp.close(),
            dp.volume(),
        );
    }

    if result.data.len() > 10 {
        println!("  … {} rows omitted …", result.data.len() - 10);
    }

    println!("══ Last 5 bars ══");
    println!(
        "  {:>12} {:>10} {:>10} {:>10} {:>10} {:>12}",
        "Date", "Open", "High", "Low", "Close", "Volume"
    );
    for dp in result
        .data
        .iter()
        .rev()
        .take(5)
        .collect::<Vec<_>>()
        .iter()
        .rev()
    {
        let dt = dp.datetime();
        println!(
            "  {:>12} {:>10.2} {:>10.2} {:>10.2} {:>10.2} {:>12.0}",
            dt.format("%Y-%m-%d"),
            dp.open(),
            dp.high(),
            dp.low(),
            dp.close(),
            dp.volume(),
        );
    }

    Ok(())
}
