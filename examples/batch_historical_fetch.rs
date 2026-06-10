//! Batch historical data fetch example.
//!
//! Demonstrates how to use `HistoricalClient::retrieve_batch()` to fetch
//! OHLCV chart data for multiple symbols concurrently, controlled by a
//! semaphore to avoid overwhelming the TradingView data server.
//!
//! # Usage
//!
//! Set `TV_AUTH_TOKEN` in your environment or a `.env` file, then run:
//!
//! ```bash
//! cargo run --example batch_historical_fetch
//! ```
//!
//! Customise with environment variables:
//!
//! ```bash
//! TV_SYMBOLS="AAPL,NASDAQ:MSFT,NASDAQ:GOOGL,NASDAQ:TSLA,NASDAQ" \
//! TV_INTERVAL=1D \
//! TV_NUM_BARS=100 \
//! TV_CONCURRENCY=4 \
//! cargo run --example batch_historical_fetch
//! ```

use std::env;

use tradingview::{
    DataServer, Interval,
    historical::{BatchConfig, BatchResult, HistoricalClient},
    prelude::OHLCV,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load .env and initialise tracing.
    let _ = dotenv::dotenv();
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    // ── Configuration ──────────────────────────────────────────────────
    let auth_token = env::var("TV_AUTH_TOKEN").unwrap_or_else(|_| {
        eprintln!("TV_AUTH_TOKEN not set — using anonymous access (may fail)");
        "unauthorized_user_token".to_string()
    });

    // Parse symbols: "AAPL,NASDAQ:MSFT,NASDAQ" or "AAPL,NASDAQ;MSFT,NASDAQ"
    let symbols_raw = env::var("TV_SYMBOLS").unwrap_or_else(|_| "AAPL:NASDAQ;FPT:HOSE".to_string());

    let mut symbols: Vec<(String, String)> = Vec::new();
    for pair_str in symbols_raw.split(|c| c == ',' || c == ';') {
        let parts: Vec<&str> = pair_str.splitn(2, ':').collect();
        match parts.as_slice() {
            [sym, exch] => symbols.push((sym.to_string(), exch.to_string())),
            [sym] => symbols.push((sym.to_string(), "NASDAQ".to_string())),
            _ => continue,
        }
    }

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

    let max_concurrency: usize = env::var("TV_CONCURRENCY")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(4);

    let server = env::var("TV_SERVER")
        .map(|s| match s.as_str() {
            "pro" => DataServer::ProData,
            "data" => DataServer::Data,
            _ => DataServer::ProData,
        })
        .unwrap_or(DataServer::ProData);

    // ── Build config ───────────────────────────────────────────────────
    let config = BatchConfig {
        max_concurrency,
        per_symbol_timeout: std::time::Duration::from_secs(30),
    };

    println!("Batch historical fetch");
    println!("  Server:       {server:?}");
    println!("  Interval:     {interval:?}");
    println!("  Bars/symbol:  {num_bars}");
    println!("  Concurrency:  {max_concurrency}");
    println!("  Symbols ({}) :", symbols.len());
    for (sym, exch) in &symbols {
        println!("    {exch}:{sym}");
    }
    println!();

    // ── Execute batch ──────────────────────────────────────────────────
    let client = HistoricalClient::new(&auth_token, server);
    let result = client
        .retrieve_batch(&symbols, interval, Some(num_bars), config)
        .await;

    // ── Display results ─────────────────────────────────────────────────
    print_batch_summary(&result);

    if !result.successful.is_empty() {
        println!("══ Successful symbols ══");
        for entry in &result.successful {
            if let Ok(hr) = &entry.result {
                let bars = hr.data.len();
                let first = hr
                    .first_datetime()
                    .map(|d| d.format("%Y-%m-%d").to_string());
                let last = hr.last_datetime().map(|d| d.format("%Y-%m-%d").to_string());
                println!(
                    "  {exch}:{sym}  bars={bars}  range={first}..{last}  elapsed={elapsed:?}",
                    exch = entry.exchange,
                    sym = entry.symbol,
                    first = first.unwrap_or_else(|| "N/A".into()),
                    last = last.unwrap_or_else(|| "N/A".into()),
                    elapsed = hr.elapsed,
                );

                // Print first 3 bars for quick preview.
                for dp in hr.data.iter().take(3) {
                    let dt = dp.datetime();
                    println!(
                        "    {:>12}  O={:>10.2} H={:>10.2} L={:>10.2} C={:>10.2} V={:>12.0}",
                        dt.format("%Y-%m-%d"),
                        dp.open(),
                        dp.high(),
                        dp.low(),
                        dp.close(),
                        dp.volume(),
                    );
                }
            }
        }
    }

    if !result.failed.is_empty() {
        println!();
        println!("══ Failed symbols ══");
        for entry in &result.failed {
            println!(
                "  {exch}:{sym}  error: {err}",
                exch = entry.exchange,
                sym = entry.symbol,
                err = entry.result.as_ref().unwrap_err(),
            );
        }
    }

    Ok(())
}

/// Print a human-readable summary of the batch result.
fn print_batch_summary(result: &BatchResult) {
    println!();
    println!("══ Batch Result ══");
    println!("  Total requested:  {}", result.total_requested);
    println!("  Successful:       {}", result.success_count());
    println!("  Failed:           {}", result.failure_count());
    println!("  Elapsed:          {:?}", result.elapsed);

    let total_bars: usize = result
        .successful
        .iter()
        .filter_map(|r| r.result.as_ref().ok())
        .map(|hr| hr.total_bars_received)
        .sum();

    println!("  Total bars:       {total_bars}");
    println!();
}
