//! Historical data fetch example.
//!
//! Demonstrates how to use `HistoricalClient` to retrieve OHLCV chart data
//! from TradingView and write the results to a CSV file.
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
//!
//! The output CSV filename defaults to `<SYMBOL>_<EXCHANGE>_<INTERVAL>.csv`
//! (e.g. `AAPL_NASDAQ_1D.csv`).  Override with the `TV_CSV_OUTPUT` env var:
//!
//! ```bash
//! TV_CSV_OUTPUT=my_data.csv cargo run --example historical_data_fetch
//! ```

use std::{env, path::PathBuf};

use tracing::info;
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
        eprintln!("TV_AUTH_TOKEN not set — using anonymous access (may fail)");
        "unauthorized_user_token".to_string()
    });

    let symbol = env::var("TV_SYMBOL").unwrap_or_else(|_| "AAPL".to_string());
    let exchange = env::var("TV_EXCHANGE").unwrap_or_else(|_| "NASDAQ".to_string());
    let interval = env::var("TV_INTERVAL")
        .map(|s| match s.as_str() {
            "1m" => Interval::OneMinute,
            "5m" => Interval::FiveMinutes,
            "15m" => Interval::FifteenMinutes,
            "30m" => Interval::ThirtyMinutes,
            "1h" => Interval::OneHour,
            "4h" => Interval::FourHours,
            "1W" => Interval::OneWeek,
            "1M" => Interval::OneMonth,
            _ => Interval::OneDay,
        })
        .unwrap_or(Interval::OneDay);

    let num_bars: u64 = env::var("TV_NUM_BARS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(100);

    let csv_output = env::var("TV_CSV_OUTPUT").unwrap_or_else(|_| {
        let interval_label = match interval {
            Interval::OneSecond => "1s",
            Interval::FiveSeconds => "5s",
            Interval::TenSeconds => "10s",
            Interval::FifteenSeconds => "15s",
            Interval::ThirtySeconds => "30s",
            Interval::OneMinute => "1m",
            Interval::ThreeMinutes => "3m",
            Interval::FiveMinutes => "5m",
            Interval::FifteenMinutes => "15m",
            Interval::ThirtyMinutes => "30m",
            Interval::FortyFiveMinutes => "45m",
            Interval::OneHour => "1h",
            Interval::TwoHours => "2h",
            Interval::FourHours => "4h",
            Interval::OneDay => "1D",
            Interval::OneWeek => "1W",
            Interval::OneMonth => "1M",
            Interval::OneQuarter => "1Q",
            Interval::SixMonths => "6M",
            Interval::Yearly => "1Y",
        };
        format!("{symbol}_{exchange}_{interval_label}.csv")
    });

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
        .interval(interval)
        .num_bars(num_bars)
        .timeout(std::time::Duration::from_secs(30))
        .build();

    println!("Fetching {exchange}:{symbol}  interval={interval:?}  bars={num_bars} …");

    // ── Execute ────────────────────────────────────────────────────────
    let client = HistoricalClient::new(&auth_token, server);
    let result = client.retrieve(request).await?;

    let data = &result.data;
    if data.is_empty() {
        eprintln!("No data returned — nothing to write.");
        return Ok(());
    }

    // ── Write CSV ──────────────────────────────────────────────────────
    let output_path = PathBuf::from(&csv_output);
    let mut wtr = csv::Writer::from_path(&output_path)?;

    // Header row
    wtr.write_record(&["datetime", "open", "high", "low", "close", "volume"])?;

    for dp in data {
        let dt = dp.datetime();
        wtr.write_record(&[
            dt.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
            dp.open().to_string(),
            dp.high().to_string(),
            dp.low().to_string(),
            dp.close().to_string(),
            dp.volume().to_string(),
        ])?;
    }

    wtr.flush()?;
    let bytes_written = std::fs::metadata(&output_path)
        .map(|m| m.len())
        .unwrap_or(0);

    info!("Wrote {} rows to {}", data.len(), output_path.display());

    // ── Display summary ────────────────────────────────────────────────
    println!();
    println!("══ Historical Data Result ══");
    println!("  Symbol:        {}", result.symbol_info.name);
    println!("  Exchange:      {}", result.symbol_info.exchange);
    println!("  Description:   {}", result.symbol_info.description);
    println!("  Currency:      {}", result.symbol_info.currency_code);
    println!("  Market type:   {}", result.symbol_info.market_type);
    println!("  ─────────────────────────────");
    println!("  Bars received: {}", result.total_bars_received);
    println!("  Bars returned: {}", data.len());
    println!("  Replay used:   {}", result.replay_used);
    println!("  Elapsed:       {:?}", result.elapsed);
    println!();
    println!(
        "  CSV output:    {}  ({bytes_written} bytes)",
        output_path.display()
    );

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
    for dp in data.iter().take(5) {
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

    if data.len() > 10 {
        println!("  … {} rows omitted …", data.len() - 10);
    }

    println!("══ Last 5 bars ══");
    println!(
        "  {:>12} {:>10} {:>10} {:>10} {:>10} {:>12}",
        "Date", "Open", "High", "Low", "Close", "Volume"
    );
    for dp in data.iter().rev().take(5).collect::<Vec<_>>().iter().rev() {
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
