//! Full fundamental Pine studies fetch and export example.
//!
//! Fetches all available fundamental Pine studies for a stock from TradingView's
//! catalog, evaluates them over WebSocket concurrently with a bounded semaphore,
//! and exports a normalized, lossless CSV containing all metrics.
//!
//! # Features
//!
//! - **Registry Discovery**: Fetches live date-versioned catalog (1400+ studies)
//!   from TradingView's Pine facade.
//! - **Bounded Concurrency**: Bounded async workers preserve TradingView rate limits
//!   via an asynchronous [`tokio::sync::Semaphore`].
//! - **Lossless Long-Form CSV**: Normalized output format preserving multi-plot values
//!   and data point indices without data loss.
//! - **Full Catalog Accounting**: Explicitly records empty results (0 points) and
//!   per-study errors so the CSV completely represents every catalog item.
//! - **Deterministic Order**: Results are sorted back into canonical registry order
//!   before writing, ensuring repeatable runs.
//!
//! # Environment Variables
//!
//! | Variable                     | Default                                           | Description                                  |
//! |------------------------------|---------------------------------------------------|----------------------------------------------|
//! | `TV_SYMBOL`                  | `AAPL`                                            | Ticker symbol to query                       |
//! | `TV_EXCHANGE`                | `NASDAQ`                                          | Exchange code                                |
//! | `TV_AUTH_TOKEN`              | `unauthorized_user_token`                         | TradingView session token                    |
//! | `TV_FUNDAMENTAL_CSV`         | `<EXCHANGE>_<SYMBOL>_fundamentals_<DATE>.csv`     | Output CSV path                              |
//! | `TV_FUNDAMENTAL_CONCURRENCY` | `4`                                               | Maximum concurrent WebSocket study requests  |
//! | `TV_NUM_BARS`                | `100`                                             | Base bar count for each study                |
//! | `TV_SERVER`                  | `data`                                            | TradingView data server (`data` or `pro`)    |
//! | `TV_TIMEOUT_SECS`            | `20`                                              | Per-study request timeout in seconds         |
//! | `TV_MAX_ENTRIES`             | *(none)*                                          | Optional max entries to fetch (for testing)  |
//! | `TV_FUND_ID`                 | *(none)*                                          | Optional filter by fund_id substring         |
//! | `TV_ERROR_CSV`               | *(none)*                                          | Optional secondary CSV path for errors only  |
//!
//! # Usage
//!
//! ```bash
//! # Run with defaults (NASDAQ:AAPL)
//! cargo run --example full_fundamental_fetch
//!
//! # Custom symbol and concurrency
//! TV_SYMBOL=MSFT TV_EXCHANGE=NASDAQ TV_FUNDAMENTAL_CONCURRENCY=6 cargo run --example full_fundamental_fetch
//!
//! # Smoke test first 5 studies
//! TV_MAX_ENTRIES=5 cargo run --example full_fundamental_fetch
//! ```

use std::{
    env,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tracing::{debug, info, warn};
use tradingview::{
    DataServer, Interval,
    chart::DataPoint,
    fundamental::{FundamentalRegistryEntry, fetch_fundamental_registry},
    study::{StudyClient, StudyConfiguration, StudyRequest},
};

/// Fetch result status for a single fundamental registry entry.
#[derive(Debug)]
enum EntryStatus {
    /// Study returned one or more data points.
    Success(Vec<DataPoint>),
    /// Study completed successfully but returned zero data points for this symbol.
    Empty,
    /// Metadata fetch or study evaluation failed.
    Error(String),
}

/// Executes a single fundamental Pine study retrieval for an entry.
async fn fetch_study_data(
    client: &StudyClient,
    entry: &FundamentalRegistryEntry,
    symbol: &str,
    exchange: &str,
    num_bars: u64,
    timeout: Duration,
) -> EntryStatus {
    // 1. Fetch indicator metadata using exact stored script ID and version.
    let indicator = match entry.fetch_indicator(None).await {
        Ok(ind) => ind,
        Err(err) => {
            return EntryStatus::Error(format!("indicator metadata fetch failed: {err}"));
        }
    };

    // 2. Build study request with Pine configuration.
    let request = StudyRequest::builder()
        .symbol(symbol)
        .exchange(exchange)
        .interval(Interval::OneDay)
        .base_bar_count(num_bars)
        .study(StudyConfiguration::Pine(indicator))
        .timeout(timeout)
        .build();

    // 3. Retrieve study data via WebSocket client.
    match client.retrieve(request).await {
        Ok(result) => {
            if result.data.is_empty() {
                EntryStatus::Empty
            } else {
                EntryStatus::Success(result.data)
            }
        }
        Err(err) => EntryStatus::Error(format!("study retrieval failed: {err}")),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();

    tracing_subscriber::fmt::init();

    // ── Configuration ──────────────────────────────────────────────────────────
    let symbol = env::var("TV_SYMBOL").unwrap_or_else(|_| "AAPL".to_string());
    let exchange = env::var("TV_EXCHANGE").unwrap_or_else(|_| "NASDAQ".to_string());
    let auth_token =
        env::var("TV_AUTH_TOKEN").unwrap_or_else(|_| "unauthorized_user_token".to_string());

    let concurrency: usize = env::var("TV_FUNDAMENTAL_CONCURRENCY")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|&c| c > 0)
        .unwrap_or(4);

    let num_bars: u64 = env::var("TV_NUM_BARS")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|&b| b > 0)
        .unwrap_or(100);

    let timeout_secs: u64 = env::var("TV_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|&t| t > 0)
        .unwrap_or(20);
    let request_timeout = Duration::from_secs(timeout_secs);

    let server = env::var("TV_SERVER")
        .map(|s| match s.to_lowercase().as_str() {
            "pro" | "prodata" => DataServer::ProData,
            _ => DataServer::Data,
        })
        .unwrap_or(DataServer::Data);

    let max_entries: Option<usize> = env::var("TV_MAX_ENTRIES")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|&m| m > 0);

    let fund_id_filter: Option<String> = env::var("TV_FUND_ID")
        .ok()
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty());

    // ── Step 1: Fetch Fundamental Registry ─────────────────────────────────────
    info!("Fetching fundamental study registry from TradingView...");
    let start_registry = Instant::now();
    let registry = fetch_fundamental_registry().await?;
    let registry_elapsed = start_registry.elapsed();

    let registry_date_str = registry.date().format("%Y-%m-%d").to_string();
    let schema_version_str = registry.version.to_string();

    info!(
        "Registry fetched in {:?}: {} total studies registered (date: {}, schema v{})",
        registry_elapsed,
        registry.len(),
        registry_date_str,
        schema_version_str
    );

    // ── Determine Output Filename ──────────────────────────────────────────────
    let default_csv_name = format!("{exchange}_{symbol}_fundamentals_{registry_date_str}.csv");
    let csv_output = env::var("TV_FUNDAMENTAL_CSV").unwrap_or(default_csv_name);
    let output_path = PathBuf::from(&csv_output);

    // Filter or slice catalog if requested
    let total_catalog_len = registry.len();
    let mut entries_to_process = registry.entries;
    if let Some(filter) = &fund_id_filter {
        info!("Filtering catalog for fund_id containing '{filter}'");
        entries_to_process.retain(|e| e.fund_id.as_str().to_lowercase().contains(filter));
    }
    if let Some(limit) = max_entries {
        info!(
            "Limiting run to first {limit} entries out of {}",
            entries_to_process.len()
        );
        entries_to_process.truncate(limit);
    }

    let total_to_process = entries_to_process.len();
    info!(
        "Starting retrieval for {exchange}:{symbol} (concurrency={concurrency}, num_bars={num_bars}, timeout={timeout_secs}s, entries={total_to_process})"
    );

    // ── Step 2: Concurrent Evaluation ──────────────────────────────────────────
    let client = Arc::new(StudyClient::new(auth_token, server));
    let semaphore = Arc::new(Semaphore::new(concurrency));
    let mut join_set = JoinSet::new();

    let start_retrievals = Instant::now();

    for (idx, entry) in entries_to_process.into_iter().enumerate() {
        let sem = Arc::clone(&semaphore);
        let cli = Arc::clone(&client);
        let sym = symbol.clone();
        let ex = exchange.clone();

        join_set.spawn(async move {
            let permit = match sem.acquire().await {
                Ok(p) => p,
                Err(_) => {
                    return (
                        idx,
                        entry,
                        EntryStatus::Error("concurrency semaphore closed".to_string()),
                    );
                }
            };

            debug!(
                idx = idx,
                fund_id = %entry.fund_id,
                script_id = %entry.script_id,
                "fetching study"
            );

            let status = fetch_study_data(&cli, &entry, &sym, &ex, num_bars, request_timeout).await;

            drop(permit);
            (idx, entry, status)
        });
    }

    // Drain completed tasks into vector
    let mut results = Vec::with_capacity(total_to_process);
    let mut processed_count = 0;

    while let Some(res) = join_set.join_next().await {
        match res {
            Ok((idx, entry, status)) => {
                processed_count += 1;
                if processed_count % 50 == 0 || processed_count == total_to_process {
                    info!(
                        "Progress: {}/{} studies processed ({:.1}%)",
                        processed_count,
                        total_to_process,
                        (processed_count as f64 / total_to_process as f64) * 100.0
                    );
                }
                results.push((idx, entry, status));
            }
            Err(err) => {
                warn!("Background task panicked or was cancelled: {err}");
            }
        }
    }

    let retrieval_elapsed = start_retrievals.elapsed();
    info!("All {processed_count} studies evaluated in {retrieval_elapsed:?}");

    // ── Step 3: Sort for Deterministic CSV Output ──────────────────────────────
    // Sorting by original canonical index guarantees exact catalog order.
    results.sort_by_key(|(idx, _, _)| *idx);

    // ── Step 4: Write Normalized Long-Form CSV ─────────────────────────────────
    info!("Writing results to CSV: {}", output_path.display());
    let mut wtr = csv::Writer::from_path(&output_path)?;

    // Write header
    wtr.write_record([
        "registry_date",
        "schema_version",
        "fund_id",
        "script_name",
        "category",
        "financial_period",
        "script_id",
        "script_version",
        "symbol",
        "exchange",
        "status",
        "error",
        "timestamp",
        "index",
        "plot_index",
        "value",
    ])?;

    let mut success_count = 0usize;
    let mut empty_count = 0usize;
    let mut error_count = 0usize;
    let mut total_points_count = 0usize;
    let mut total_rows_written = 0usize;

    for (_idx, entry, status) in &mut results {
        let period_str = entry
            .financial_period
            .as_ref()
            .map(|p| p.to_string())
            .unwrap_or_default();
        let category_str = entry.fundamental_category.as_deref().unwrap_or("");

        match status {
            EntryStatus::Success(points) => {
                success_count += 1;
                total_points_count += points.len();

                // Sort points by (timestamp, index) for deterministic intra-study ordering
                points.sort_by(|a, b| {
                    let ts_a = a.value.first().copied().map(|v| v as i64).unwrap_or(0);
                    let ts_b = b.value.first().copied().map(|v| v as i64).unwrap_or(0);
                    ts_a.cmp(&ts_b).then_with(|| a.index.cmp(&b.index))
                });

                for dp in points.iter() {
                    let index_str = dp.index.to_string();

                    if dp.value.is_empty() {
                        wtr.write_record([
                            &registry_date_str,
                            &schema_version_str,
                            entry.fund_id.as_str(),
                            entry.script_name.as_str(),
                            category_str,
                            &period_str,
                            entry.script_id.as_str(),
                            entry.script_version.as_str(),
                            &symbol,
                            &exchange,
                            "success",
                            "",
                            "",
                            &index_str,
                            "",
                            "",
                        ])?;
                        total_rows_written += 1;
                    } else if dp.value.len() == 1 {
                        let ts_str = (dp.value[0] as i64).to_string();
                        wtr.write_record([
                            &registry_date_str,
                            &schema_version_str,
                            entry.fund_id.as_str(),
                            entry.script_name.as_str(),
                            category_str,
                            &period_str,
                            entry.script_id.as_str(),
                            entry.script_version.as_str(),
                            &symbol,
                            &exchange,
                            "success",
                            "",
                            &ts_str,
                            &index_str,
                            "",
                            "",
                        ])?;
                        total_rows_written += 1;
                    } else {
                        let ts_str = (dp.value[0] as i64).to_string();
                        // Long-form row for each plot value starting at index 1
                        for (plot_idx, &val) in dp.value[1..].iter().enumerate() {
                            let plot_idx_str = plot_idx.to_string();
                            let val_str = val.to_string();
                            wtr.write_record([
                                &registry_date_str,
                                &schema_version_str,
                                entry.fund_id.as_str(),
                                entry.script_name.as_str(),
                                category_str,
                                &period_str,
                                entry.script_id.as_str(),
                                entry.script_version.as_str(),
                                &symbol,
                                &exchange,
                                "success",
                                "",
                                &ts_str,
                                &index_str,
                                &plot_idx_str,
                                &val_str,
                            ])?;
                            total_rows_written += 1;
                        }
                    }
                }
            }
            EntryStatus::Empty => {
                empty_count += 1;
                wtr.write_record([
                    &registry_date_str,
                    &schema_version_str,
                    entry.fund_id.as_str(),
                    entry.script_name.as_str(),
                    category_str,
                    &period_str,
                    entry.script_id.as_str(),
                    entry.script_version.as_str(),
                    &symbol,
                    &exchange,
                    "empty",
                    "",
                    "",
                    "",
                    "",
                    "",
                ])?;
                total_rows_written += 1;
            }
            EntryStatus::Error(err_msg) => {
                error_count += 1;
                wtr.write_record([
                    &registry_date_str,
                    &schema_version_str,
                    entry.fund_id.as_str(),
                    entry.script_name.as_str(),
                    category_str,
                    &period_str,
                    entry.script_id.as_str(),
                    entry.script_version.as_str(),
                    &symbol,
                    &exchange,
                    "error",
                    err_msg.as_str(),
                    "",
                    "",
                    "",
                    "",
                ])?;
                total_rows_written += 1;
            }
        }
    }

    wtr.flush()?;

    // ── Optional Separate Error CSV ────────────────────────────────────────────
    if let Some(err_path_str) = env::var("TV_ERROR_CSV").ok().filter(|s| !s.is_empty()) {
        let err_path = PathBuf::from(err_path_str);
        let mut err_wtr = csv::Writer::from_path(&err_path)?;
        err_wtr.write_record([
            "registry_date",
            "fund_id",
            "script_name",
            "category",
            "financial_period",
            "script_id",
            "script_version",
            "symbol",
            "exchange",
            "error",
        ])?;

        for (_, entry, status) in &results {
            if let EntryStatus::Error(err_msg) = status {
                let period_str = entry
                    .financial_period
                    .as_ref()
                    .map(|p| p.to_string())
                    .unwrap_or_default();
                err_wtr.write_record([
                    &registry_date_str,
                    entry.fund_id.as_str(),
                    entry.script_name.as_str(),
                    entry.fundamental_category.as_deref().unwrap_or(""),
                    &period_str,
                    entry.script_id.as_str(),
                    entry.script_version.as_str(),
                    &symbol,
                    &exchange,
                    err_msg.as_str(),
                ])?;
            }
        }
        err_wtr.flush()?;
        info!("Secondary error CSV written to {}", err_path.display());
    }

    let file_size_bytes = std::fs::metadata(&output_path)
        .map(|m| m.len())
        .unwrap_or(0);

    // ── Print Summary Counts ───────────────────────────────────────────────────
    println!();
    println!("════════════════════════════════════════════════════════════════");
    println!("             TradingView Fundamental Fetch Summary              ");
    println!("════════════════════════════════════════════════════════════════");
    println!("  Target Symbol:        {exchange}:{symbol}");
    println!("  Registry Date:        {registry_date_str} (schema v{schema_version_str})");
    println!("  Total Catalog Size:   {total_catalog_len} studies");
    println!("  Entries Processed:    {total_to_process}");
    println!("  Concurrency Limit:    {concurrency}");
    println!("  Base Bar Count:       {num_bars}");
    println!("  Per-Study Timeout:    {timeout_secs}s");
    println!("────────────────────────────────────────────────────────────────");
    println!("  Successful Studies:   {success_count}");
    println!("  Empty Studies:        {empty_count}");
    println!("  Failed Studies:       {error_count}");
    println!("  Total Data Points:    {total_points_count}");
    println!("  Total CSV Rows:       {total_rows_written}");
    println!("────────────────────────────────────────────────────────────────");
    println!("  Output CSV File:      {}", output_path.display());
    println!("  File Size:            {file_size_bytes} bytes");
    println!("  Total Elapsed Time:   {:?}", start_registry.elapsed());
    println!("════════════════════════════════════════════════════════════════");
    println!();

    Ok(())
}
