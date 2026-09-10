//! Full fundamental Pine studies fetch and export example.
//!
//! Evaluates all fundamental studies for a stock using [`fetch_full_fundamentals`]
//! (or with optional smoke-test configuration) and exports the results to a lossless CSV.
//!
//! # Usage
//!
//! ```bash
//! # Default (AAPL)
//! cargo run --example full_fundamental_fetch
//!
//! # Custom stock code (bare or canonical EXCHANGE:SYMBOL)
//! cargo run --example full_fundamental_fetch -- AAPL
//! cargo run --example full_fundamental_fetch -- NASDAQ:MSFT
//! cargo run --example full_fundamental_fetch -- HOSE:FPT
//! ```

use std::env;
use std::path::PathBuf;
use std::time::Duration;

use tracing::info;
use tradingview::{
    DataServer,
    fundamental::{
        FullFundamentalConfig, fetch_full_fundamentals, fetch_full_fundamentals_with_config,
    },
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();
    tracing_subscriber::fmt::init();

    // 1. Resolve target stock: CLI positional argument -> TV_SYMBOL env -> default "AAPL"
    let stock = env::args()
        .nth(1)
        .or_else(|| env::var("TV_SYMBOL").ok())
        .unwrap_or_else(|| "AAPL".to_string());

    // 2. Optional test/tuning hooks via environment variables
    let max_entries = env::var("TV_MAX_ENTRIES").ok().and_then(|v| v.parse().ok());
    let fund_id_filter = env::var("TV_FUND_ID").ok();
    let concurrency = env::var("TV_FUNDAMENTAL_CONCURRENCY")
        .ok()
        .and_then(|v| v.parse().ok());
    let num_bars = env::var("TV_NUM_BARS").ok().and_then(|v| v.parse().ok());
    let timeout_secs: Option<u64> = env::var("TV_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse().ok());
    let auth_token = env::var("TV_AUTH_TOKEN").ok();
    let server = env::var("TV_SERVER")
        .ok()
        .map(|s| match s.to_lowercase().as_str() {
            "pro" | "prodata" => DataServer::ProData,
            _ => DataServer::Data,
        });

    let has_custom_config = max_entries.is_some()
        || fund_id_filter.is_some()
        || concurrency.is_some()
        || num_bars.is_some()
        || timeout_secs.is_some()
        || auth_token.is_some()
        || server.is_some();

    info!("Starting fundamental studies fetch for '{stock}'");

    // 3. Execute fetch via library public API
    let result = if has_custom_config {
        let mut config = FullFundamentalConfig::default();
        if let Some(m) = max_entries {
            config = config.max_entries(Some(m));
        }
        if let Some(f) = fund_id_filter {
            config = config.fund_id_filter(Some(f));
        }
        if let Some(c) = concurrency {
            config = config.concurrency(c);
        }
        if let Some(b) = num_bars {
            config = config.num_bars(b);
        }
        if let Some(t) = timeout_secs {
            config = config.request_timeout(Duration::from_secs(t));
        }
        if let Some(tok) = auth_token {
            config = config.auth_token(tok);
        }
        if let Some(srv) = server {
            config = config.server(srv);
        }
        fetch_full_fundamentals_with_config(&stock, config).await?
    } else {
        fetch_full_fundamentals(&stock).await?
    };

    // 4. Determine output CSV path and write
    let date_str = result.registry_date.format("%Y-%m-%d").to_string();
    let default_csv_name = format!(
        "{}_{}_fundamentals_{date_str}.csv",
        result.effective_exchange(),
        result.symbol_ticker()
    );
    let csv_output = env::var("TV_FUNDAMENTAL_CSV").unwrap_or(default_csv_name);
    let output_path = PathBuf::from(&csv_output);

    result.write_csv(&output_path)?;

    // Optional error CSV
    if let Some(err_csv) = env::var("TV_ERROR_CSV").ok().filter(|s| !s.is_empty()) {
        result.write_error_csv(&err_csv)?;
        info!("Secondary error CSV written to {err_csv}");
    }

    let file_size_bytes = std::fs::metadata(&output_path)
        .map(|m| m.len())
        .unwrap_or(0);

    // 5. Print summary
    println!();
    println!("════════════════════════════════════════════════════════════════");
    println!("             TradingView Fundamental Fetch Summary              ");
    println!("════════════════════════════════════════════════════════════════");
    println!("  Target Stock:         {}", result.canonical_id());
    println!(
        "  Registry Date:        {date_str} (schema v{})",
        result.schema_version
    );
    println!(
        "  Total Catalog Size:   {} studies",
        result.total_catalog_size
    );
    println!("  Studies Evaluated:    {}", result.total_studies());
    println!("────────────────────────────────────────────────────────────────");
    println!("  Successful Studies:   {}", result.success_count());
    println!("  Empty Studies:        {}", result.empty_count());
    println!("  Failed Studies:       {}", result.error_count());
    println!("  Total Data Points:    {}", result.total_data_points());
    println!("  Total CSV Rows:       {}", result.total_csv_rows());
    println!("────────────────────────────────────────────────────────────────");
    println!("  Output CSV File:      {}", output_path.display());
    println!("  File Size:            {file_size_bytes} bytes");
    println!("  Total Elapsed Time:   {:?}", result.elapsed);
    println!("════════════════════════════════════════════════════════════════");
    println!();

    Ok(())
}
