//! Full fundamental Pine studies retrieval and export workflow.
//!
//! Evaluates all available fundamental Pine studies for a stock from TradingView's
//! catalog over WebSocket with bounded concurrency, returning a typed [`FullFundamentalResult`]
//! that owns all resolved symbol data, study outcomes, summary metrics, and optional CSV export.
//!
//! # Resolution Semantics
//!
//! Stock identifiers can be provided either as:
//! - **Canonical `EXCHANGE:SYMBOL`** (e.g., `"NASDAQ:AAPL"`, `"HOSE:FPT"`, `"TWSE:2330"`):
//!   Bypasses search directly and uses the explicit exchange and symbol.
//! - **Bare ticker** (e.g., `"AAPL"`, `"fpt"`, `"2330"`):
//!   Resolves via [`advanced_search_symbol`] with `MarketType::Stocks(StocksType::All)`.
//!   Uses TradingView's search rank order, selecting the first exact case-insensitive match
//!   where `market_type == "stock"` and `type_specs` contains `"common"`. If multiple exact
//!   matches exist without a determinable common stock winner, an ambiguity error is returned
//!   listing candidate exchange IDs.
//!
//! # Catalog Filtering
//!
//! TradingView's fundamental Pine registry contains both stock fundamentals (prefixed with
//! `STD;Fund_`) and crypto fundamentals (prefixed with `STD;CryptoFund_`). Full stock fetches
//! automatically filter to `STD;Fund_` entries.
//!
//! # Example
//!
//! ```no_run
//! use tradingview::fundamental::fetch_full_fundamentals;
//!
//! # async fn run() -> tradingview::Result<()> {
//! // Fetch full fundamentals with one argument:
//! let result = fetch_full_fundamentals("AAPL").await?;
//!
//! println!("Resolved: {}", result.canonical_id());
//! println!(
//!     "Studies: {} total ({} succeeded, {} empty, {} failed)",
//!     result.total_studies(),
//!     result.success_count(),
//!     result.empty_count(),
//!     result.error_count()
//! );
//!
//! // Export lossless long-form CSV:
//! result.write_csv("AAPL_fundamentals.csv")?;
//! # Ok(())
//! # }
//! ```

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tracing::{debug, info, instrument};
use ustr::Ustr;

use crate::{
    DataServer, Error, Interval, Result, Symbol,
    chart::DataPoint,
    client::misc::advanced_search_symbol,
    fundamental::{FundamentalRegistryEntry, fetch_fundamental_registry},
    models::{MarketType, StocksType},
    study::{StudyClient, StudyConfiguration, StudyRequest},
};

/// Fetch result status for a single fundamental registry entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FundamentalEntryStatus {
    /// Study returned one or more data points.
    Success(Vec<DataPoint>),
    /// Study completed successfully but returned zero data points for this symbol.
    Empty,
    /// Metadata fetch or study evaluation failed.
    Error(String),
}

impl FundamentalEntryStatus {
    /// Returns `true` if the study succeeded with at least one data point.
    #[inline]
    #[must_use]
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success(_))
    }

    /// Returns `true` if the study succeeded but returned zero data points.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        matches!(self, Self::Empty)
    }

    /// Returns `true` if the study failed.
    #[inline]
    #[must_use]
    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error(_))
    }

    /// Returns the points slice if successful.
    #[inline]
    #[must_use]
    pub fn points(&self) -> Option<&[DataPoint]> {
        match self {
            Self::Success(pts) => Some(pts),
            _ => None,
        }
    }

    /// Returns the error message if failed.
    #[inline]
    #[must_use]
    pub fn error_message(&self) -> Option<&str> {
        match self {
            Self::Error(msg) => Some(msg.as_str()),
            _ => None,
        }
    }
}

/// Evaluated outcome for a single registered fundamental Pine study.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FullFundamentalEntryResult {
    /// Fundamental study registry entry metadata.
    pub entry: FundamentalRegistryEntry,
    /// Evaluation outcome.
    pub status: FundamentalEntryStatus,
}

impl FullFundamentalEntryResult {
    /// Returns the metric canonical fund ID (e.g., `"total_revenue_fy"`).
    #[inline]
    #[must_use]
    pub fn fund_id(&self) -> &str {
        self.entry.fund_id.as_str()
    }

    /// Returns the Pine Script study ID (e.g., `"STD;Fund_total_revenue_fy"`).
    #[inline]
    #[must_use]
    pub fn script_id(&self) -> &str {
        self.entry.script_id.as_str()
    }

    /// Returns `true` if this study returned data points.
    #[inline]
    #[must_use]
    pub fn is_success(&self) -> bool {
        self.status.is_success()
    }

    /// Returns `true` if this study returned no data points.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.status.is_empty()
    }

    /// Returns `true` if this study encountered an error.
    #[inline]
    #[must_use]
    pub fn is_error(&self) -> bool {
        self.status.is_error()
    }
}

/// Configuration options for full fundamental study retrieval.
#[derive(Debug, Clone)]
pub struct FullFundamentalConfig {
    /// TradingView session token or anonymous user token.
    pub auth_token: String,
    /// TradingView WebSocket data server (`Data` or `ProData`).
    pub server: DataServer,
    /// Base bar count requested for each study.
    pub num_bars: u64,
    /// Maximum concurrent WebSocket study requests.
    ///
    /// **Default: 2.** Evaluating 1340+ fundamental studies over WebSocket
    /// requires conservative concurrency to avoid TradingView rate limits and connection drops.
    pub concurrency: usize,
    /// Per-study request timeout.
    pub request_timeout: Duration,
    /// Optional limit on the number of entries to process (useful for smoke tests).
    #[doc(hidden)]
    pub max_entries: Option<usize>,
    /// Optional case-insensitive substring filter on `fund_id` (useful for smoke tests).
    #[doc(hidden)]
    pub fund_id_filter: Option<String>,
}

impl Default for FullFundamentalConfig {
    fn default() -> Self {
        Self {
            auth_token: "unauthorized_user_token".to_string(),
            server: DataServer::Data,
            num_bars: 100,
            concurrency: 2,
            request_timeout: Duration::from_secs(20),
            max_entries: None,
            fund_id_filter: None,
        }
    }
}

impl FullFundamentalConfig {
    /// Creates a new config with default parameters.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the auth token.
    #[must_use]
    pub fn auth_token(mut self, token: impl Into<String>) -> Self {
        self.auth_token = token.into();
        self
    }

    /// Sets the data server.
    #[must_use]
    pub fn server(mut self, server: DataServer) -> Self {
        self.server = server;
        self
    }

    /// Sets the base bar count.
    #[must_use]
    pub fn num_bars(mut self, num_bars: u64) -> Self {
        self.num_bars = num_bars;
        self
    }

    /// Sets the concurrency limit.
    #[must_use]
    pub fn concurrency(mut self, concurrency: usize) -> Self {
        self.concurrency = concurrency.max(1);
        self
    }

    /// Sets the per-study request timeout.
    #[must_use]
    pub fn request_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = timeout;
        self
    }

    /// Sets an optional maximum number of entries to evaluate.
    #[doc(hidden)]
    #[must_use]
    pub fn max_entries(mut self, max: Option<usize>) -> Self {
        self.max_entries = max;
        self
    }

    /// Sets an optional substring filter on `fund_id`.
    #[doc(hidden)]
    #[must_use]
    pub fn fund_id_filter(mut self, filter: Option<String>) -> Self {
        self.fund_id_filter = filter;
        self
    }
}

/// Comprehensive results from evaluating the fundamental catalog for a stock.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FullFundamentalResult {
    /// The resolved stock symbol metadata.
    pub symbol: Symbol,
    /// UTC snapshot date of the fundamental registry.
    pub registry_date: NaiveDate,
    /// Schema version of the fundamental registry.
    pub schema_version: u32,
    /// Total unfiltered studies registered in TradingView's catalog.
    pub total_catalog_size: usize,
    /// Evaluated studies in deterministic canonical registry order.
    pub entries: Vec<FullFundamentalEntryResult>,
    /// Total wall-clock elapsed time for retrieval.
    pub elapsed: Duration,
}

impl FullFundamentalResult {
    /// Returns the canonical symbol ticker (e.g., `"AAPL"`).
    #[inline]
    #[must_use]
    pub fn symbol_ticker(&self) -> &str {
        &self.symbol.symbol
    }

    /// Returns the effective exchange code (e.g., `"NASDAQ"` or `"AMEX"`).
    #[inline]
    #[must_use]
    pub fn effective_exchange(&self) -> &str {
        if !self.symbol.prefix.is_empty() {
            &self.symbol.prefix
        } else {
            &self.symbol.exchange
        }
    }

    /// Returns the canonical identifier string `EXCHANGE:SYMBOL` (e.g., `"NASDAQ:AAPL"`).
    #[inline]
    #[must_use]
    pub fn canonical_id(&self) -> String {
        format!("{}:{}", self.effective_exchange(), self.symbol_ticker())
    }

    /// Returns the number of studies evaluated.
    #[inline]
    #[must_use]
    pub fn total_studies(&self) -> usize {
        self.entries.len()
    }

    /// Returns the count of studies that successfully returned data points.
    #[inline]
    #[must_use]
    pub fn success_count(&self) -> usize {
        self.entries.iter().filter(|e| e.is_success()).count()
    }

    /// Returns the count of studies that completed with zero data points.
    #[inline]
    #[must_use]
    pub fn empty_count(&self) -> usize {
        self.entries.iter().filter(|e| e.is_empty()).count()
    }

    /// Returns the count of studies that encountered an error.
    #[inline]
    #[must_use]
    pub fn error_count(&self) -> usize {
        self.entries.iter().filter(|e| e.is_error()).count()
    }

    /// Returns the total number of individual data points collected across all successful studies.
    #[must_use]
    pub fn total_data_points(&self) -> usize {
        self.entries
            .iter()
            .filter_map(|e| e.status.points())
            .map(|pts| pts.len())
            .sum()
    }

    /// Computes the exact number of rows that will be written to the long-form CSV (excluding header).
    #[must_use]
    pub fn total_csv_rows(&self) -> usize {
        let mut rows = 0;
        for e in &self.entries {
            match &e.status {
                FundamentalEntryStatus::Success(pts) => {
                    for dp in pts {
                        if dp.value.len() <= 1 {
                            rows += 1;
                        } else {
                            rows += dp.value.len() - 1;
                        }
                    }
                }
                FundamentalEntryStatus::Empty | FundamentalEntryStatus::Error(_) => {
                    rows += 1;
                }
            }
        }
        rows
    }

    /// Serializes the results to an in-memory CSV string using the standard 16-column schema.
    pub fn to_csv_string(&self) -> Result<String> {
        let mut buf = Vec::new();
        self.write_csv_to_writer(&mut buf)?;
        String::from_utf8(buf).map_err(|e| Error::Internal(Ustr::from(&e.to_string())))
    }

    /// Writes the full fundamental results to a lossless long-form CSV file at the specified path.
    ///
    /// # CSV Schema (16 Columns)
    /// 1. `registry_date`
    /// 2. `schema_version`
    /// 3. `fund_id`
    /// 4. `script_name`
    /// 5. `category`
    /// 6. `financial_period`
    /// 7. `script_id`
    /// 8. `script_version`
    /// 9. `symbol`
    /// 10. `exchange`
    /// 11. `status` (`success`, `empty`, `error`)
    /// 12. `error`
    /// 13. `timestamp`
    /// 14. `index`
    /// 15. `plot_index`
    /// 16. `value`
    pub fn write_csv<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);
        self.write_csv_to_writer(&mut writer)?;
        writer.flush()?;
        Ok(())
    }

    /// Writes results in CSV format to any [`std::io::Write`] destination.
    pub fn write_csv_to_writer<W: Write>(&self, writer: &mut W) -> Result<()> {
        let registry_date_str = self.registry_date.format("%Y-%m-%d").to_string();
        let schema_version_str = self.schema_version.to_string();
        let symbol_str = self.symbol_ticker();
        let exchange_str = self.effective_exchange();

        // Write header
        write_csv_record(
            writer,
            &[
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
            ],
        )?;

        for item in &self.entries {
            let entry = &item.entry;
            let period_str = entry
                .financial_period
                .as_ref()
                .map(|p| p.to_string())
                .unwrap_or_default();
            let category_str = entry.fundamental_category.as_deref().unwrap_or("");

            match &item.status {
                FundamentalEntryStatus::Success(points) => {
                    for dp in points {
                        let index_str = dp.index.to_string();
                        if dp.value.is_empty() {
                            write_csv_record(
                                writer,
                                &[
                                    &registry_date_str,
                                    &schema_version_str,
                                    entry.fund_id.as_str(),
                                    entry.script_name.as_str(),
                                    category_str,
                                    &period_str,
                                    entry.script_id.as_str(),
                                    entry.script_version.as_str(),
                                    symbol_str,
                                    exchange_str,
                                    "success",
                                    "",
                                    "",
                                    &index_str,
                                    "",
                                    "",
                                ],
                            )?;
                        } else if dp.value.len() == 1 {
                            let ts_str = (dp.value[0] as i64).to_string();
                            write_csv_record(
                                writer,
                                &[
                                    &registry_date_str,
                                    &schema_version_str,
                                    entry.fund_id.as_str(),
                                    entry.script_name.as_str(),
                                    category_str,
                                    &period_str,
                                    entry.script_id.as_str(),
                                    entry.script_version.as_str(),
                                    symbol_str,
                                    exchange_str,
                                    "success",
                                    "",
                                    &ts_str,
                                    &index_str,
                                    "",
                                    "",
                                ],
                            )?;
                        } else {
                            let ts_str = (dp.value[0] as i64).to_string();
                            for (plot_idx, &val) in dp.value[1..].iter().enumerate() {
                                let plot_idx_str = plot_idx.to_string();
                                let val_str = val.to_string();
                                write_csv_record(
                                    writer,
                                    &[
                                        &registry_date_str,
                                        &schema_version_str,
                                        entry.fund_id.as_str(),
                                        entry.script_name.as_str(),
                                        category_str,
                                        &period_str,
                                        entry.script_id.as_str(),
                                        entry.script_version.as_str(),
                                        symbol_str,
                                        exchange_str,
                                        "success",
                                        "",
                                        &ts_str,
                                        &index_str,
                                        &plot_idx_str,
                                        &val_str,
                                    ],
                                )?;
                            }
                        }
                    }
                }
                FundamentalEntryStatus::Empty => {
                    write_csv_record(
                        writer,
                        &[
                            &registry_date_str,
                            &schema_version_str,
                            entry.fund_id.as_str(),
                            entry.script_name.as_str(),
                            category_str,
                            &period_str,
                            entry.script_id.as_str(),
                            entry.script_version.as_str(),
                            symbol_str,
                            exchange_str,
                            "empty",
                            "",
                            "",
                            "",
                            "",
                            "",
                        ],
                    )?;
                }
                FundamentalEntryStatus::Error(err_msg) => {
                    write_csv_record(
                        writer,
                        &[
                            &registry_date_str,
                            &schema_version_str,
                            entry.fund_id.as_str(),
                            entry.script_name.as_str(),
                            category_str,
                            &period_str,
                            entry.script_id.as_str(),
                            entry.script_version.as_str(),
                            symbol_str,
                            exchange_str,
                            "error",
                            err_msg.as_str(),
                            "",
                            "",
                            "",
                            "",
                        ],
                    )?;
                }
            }
        }

        Ok(())
    }

    /// Writes only the errored studies to a secondary CSV file.
    pub fn write_error_csv<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);
        self.write_error_csv_to_writer(&mut writer)?;
        writer.flush()?;
        Ok(())
    }

    /// Writes only the errored studies in CSV format to any [`std::io::Write`] destination.
    pub fn write_error_csv_to_writer<W: Write>(&self, writer: &mut W) -> Result<()> {
        let registry_date_str = self.registry_date.format("%Y-%m-%d").to_string();
        let symbol_str = self.symbol_ticker();
        let exchange_str = self.effective_exchange();

        write_csv_record(
            writer,
            &[
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
            ],
        )?;

        for item in &self.entries {
            if let FundamentalEntryStatus::Error(err_msg) = &item.status {
                let entry = &item.entry;
                let period_str = entry
                    .financial_period
                    .as_ref()
                    .map(|p| p.to_string())
                    .unwrap_or_default();
                write_csv_record(
                    writer,
                    &[
                        &registry_date_str,
                        entry.fund_id.as_str(),
                        entry.script_name.as_str(),
                        entry.fundamental_category.as_deref().unwrap_or(""),
                        &period_str,
                        entry.script_id.as_str(),
                        entry.script_version.as_str(),
                        symbol_str,
                        exchange_str,
                        err_msg.as_str(),
                    ],
                )?;
            }
        }

        Ok(())
    }
}

/// Helper function to write a single RFC 4180 compliant CSV record.
fn write_csv_record<W: Write>(writer: &mut W, fields: &[&str]) -> std::io::Result<()> {
    for (i, field) in fields.iter().enumerate() {
        if i > 0 {
            writer.write_all(b",")?;
        }
        if field.contains(',')
            || field.contains('"')
            || field.contains('\n')
            || field.contains('\r')
        {
            writer.write_all(b"\"")?;
            for b in field.bytes() {
                if b == b'"' {
                    writer.write_all(b"\"\"")?;
                } else {
                    writer.write_all(&[b])?;
                }
            }
            writer.write_all(b"\"")?;
        } else {
            writer.write_all(field.as_bytes())?;
        }
    }
    writer.write_all(b"\n")?;
    Ok(())
}

/// Returns `true` if this fundamental study entry belongs to the stock fundamental catalog.
///
/// TradingView registers stock fundamentals with script IDs starting with `"STD;Fund_"`,
#[inline]
pub(crate) fn is_stock_fundamental_study(entry: &FundamentalRegistryEntry) -> bool {
    entry.script_id.as_str().starts_with("STD;Fund_")
}
/// Zero-allocation case-insensitive ASCII substring search helper.
#[inline]
fn contains_ignore_ascii_case(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    if needle.len() > haystack.len() {
        return false;
    }
    haystack
        .as_bytes()
        .windows(needle.len())
        .any(|window| window.eq_ignore_ascii_case(needle.as_bytes()))
}

/// Parses an input string into canonical `(exchange, symbol)` if formatted as `"EXCHANGE:SYMBOL"`.
///
/// Returns `None` if the input is a bare ticker without a colon delimiter.
pub(crate) fn parse_canonical_symbol(input: &str) -> Option<(String, String)> {
    let trimmed = input.trim();
    if let Some((ex, sym)) = trimmed.split_once(':') {
        let ex = ex.trim();
        let sym = sym.trim();
        if !ex.is_empty() && !sym.is_empty() && !sym.contains(':') {
            return Some((ex.to_uppercase(), sym.to_uppercase()));
        }
    }
    None
}

/// Resolves a bare stock ticker from candidate search symbols.
///
/// # Selection Rules
/// 1. Filters candidates matching `symbol.eq_ignore_ascii_case(stock)` and `market_type == "stock"`.
/// 2. If no exact matches exist, returns a "not found" error.
/// 3. Prefers candidates whose `type_specs` contain `"common"` (common stock).
/// 4. If common stock matches exist, selects the first one according to TradingView's search rank.
/// 5. If no common stock matches exist:
///    - If exactly one stock match exists, selects it.
pub(crate) fn resolve_stock_from_candidates(stock: &str, candidates: &[Symbol]) -> Result<Symbol> {
    let trimmed = stock.trim();
    if trimmed.is_empty() {
        return Err(Error::Internal(Ustr::from(
            "stock identifier cannot be empty",
        )));
    }

    let exact_stock_matches: Vec<&Symbol> = candidates
        .iter()
        .filter(|s| {
            s.symbol.eq_ignore_ascii_case(trimmed) && s.market_type.eq_ignore_ascii_case("stock")
        })
        .collect();

    if exact_stock_matches.is_empty() {
        return Err(Error::Internal(Ustr::from(&format!(
            "stock symbol '{trimmed}' not found in search results"
        ))));
    }

    // Filter for common stock
    let common_stock_matches: Vec<&Symbol> = exact_stock_matches
        .iter()
        .copied()
        .filter(|s| {
            s.type_specs
                .iter()
                .any(|spec| spec.eq_ignore_ascii_case("common"))
        })
        .collect();

    if let Some(first_common) = common_stock_matches.first() {
        // First in TradingView rank order is the primary listing
        return Ok((*first_common).clone());
    }

    if exact_stock_matches.len() == 1 {
        return Ok((*exact_stock_matches[0]).clone());
    }

    // Multiple non-common exact stock matches remain: ambiguous
    let candidate_ids: Vec<String> = exact_stock_matches.iter().map(|s| s.id()).collect();
    Err(Error::Internal(Ustr::from(&format!(
        "ambiguous stock symbol '{trimmed}': multiple matching exchanges found ({}); please specify canonical 'EXCHANGE:SYMBOL'",
        candidate_ids.join(", ")
    ))))
}

/// Resolves a stock input string (canonical `EXCHANGE:SYMBOL` or bare ticker) to a [`Symbol`].
///
/// Canonical inputs bypass remote symbol search. Bare tickers query TradingView's
pub(crate) async fn resolve_stock_symbol(stock: &str) -> Result<Symbol> {
    let trimmed = stock.trim();
    if trimmed.is_empty() {
        return Err(Error::Internal(Ustr::from(
            "stock identifier cannot be empty",
        )));
    }

    if let Some((exchange, symbol)) = parse_canonical_symbol(trimmed) {
        debug!(
            canonical = %trimmed,
            exchange = %exchange,
            symbol = %symbol,
            "bypassing search for canonical exchange:symbol input"
        );
        return Ok(Symbol {
            symbol,
            exchange,
            prefix: String::new(),
            description: String::new(),
            market_type: "stock".to_string(),
            type_specs: vec!["common".to_string()],
            ..Default::default()
        });
    }

    // Reject malformed colon inputs
    if trimmed.contains(':') {
        return Err(Error::Internal(Ustr::from(&format!(
            "invalid canonical stock format '{trimmed}', expected 'EXCHANGE:SYMBOL'"
        ))));
    }

    debug!(stock = %trimmed, "searching TradingView for bare stock symbol");
    let search_res = advanced_search_symbol()
        .search(trimmed)
        .market_type(MarketType::Stocks(StocksType::All))
        .call()
        .await?;

    resolve_stock_from_candidates(trimmed, &search_res.symbols)
}

/// Retrieves all fundamental metrics for a stock from TradingView.
///
/// Takes a single stock identifier — either canonical `EXCHANGE:SYMBOL` (e.g., `"NASDAQ:AAPL"`)
/// or a bare ticker (e.g., `"AAPL"`). Bare tickers are automatically resolved to the
/// primary exchange venue via TradingView symbol search.
///
/// Uses sensible defaults:
/// - Anonymous user token
/// - Default [`DataServer::Data`]
/// - 100 historical bars
/// - Concurrency limit of 4 workers
/// - 20-second per-study timeout
///
/// # Errors
/// Returns an error if symbol resolution fails, the fundamental registry cannot be fetched,
/// or background execution terminates abnormally.
///
/// # Example
/// ```no_run
/// use tradingview::fundamental::fetch_full_fundamentals;
///
/// # async fn run() -> tradingview::Result<()> {
/// let result = fetch_full_fundamentals("AAPL").await?;
/// println!("Resolved canonical ID: {}", result.canonical_id());
/// println!("Success count: {}", result.success_count());
/// # Ok(())
/// # }
/// ```
pub async fn fetch_full_fundamentals(stock: &str) -> Result<FullFundamentalResult> {
    fetch_full_fundamentals_with_config(stock, FullFundamentalConfig::default()).await
}

/// Retrieves all fundamental metrics for a stock from TradingView with custom configuration.
///
/// Supports custom session authentication, data server, concurrency, bar count,
/// per-request timeouts, and smoke-testing filters.
#[instrument(skip(config), fields(stock = %stock))]
pub async fn fetch_full_fundamentals_with_config(
    stock: &str,
    config: FullFundamentalConfig,
) -> Result<FullFundamentalResult> {
    let start_total = Instant::now();

    // 1. Resolve stock symbol
    let symbol = resolve_stock_symbol(stock).await?;
    let sym_ticker = symbol.symbol.clone();
    let effective_ex = if !symbol.prefix.is_empty() {
        symbol.prefix.clone()
    } else {
        symbol.exchange.clone()
    };

    info!(
        canonical = %format!("{effective_ex}:{sym_ticker}"),
        "Resolved target stock"
    );

    // 2. Fetch live fundamental registry
    let start_registry = Instant::now();
    let registry = fetch_fundamental_registry().await?;
    let total_catalog_size = registry.len();
    let registry_date = registry.date();
    let schema_version = registry.version;

    debug!(
        total = total_catalog_size,
        date = %registry_date,
        version = schema_version,
        elapsed = ?start_registry.elapsed(),
        "Fetched fundamental study registry"
    );

    // 3. Filter catalog: retain only stock fundamentals (`STD;Fund_`)
    let mut entries_to_process: Vec<FundamentalRegistryEntry> = registry
        .entries
        .into_iter()
        .filter(is_stock_fundamental_study)
        .collect();

    let total_catalog_size = entries_to_process.len();

    // Apply optional substring filter
    if let Some(filter) = &config.fund_id_filter {
        let f = filter.trim();
        if !f.is_empty() {
            entries_to_process.retain(|e| contains_ignore_ascii_case(e.fund_id.as_str(), f));
        }
    }

    // Apply optional limit
    if let Some(limit) = config.max_entries {
        entries_to_process.truncate(limit);
    }

    let total_to_process = entries_to_process.len();
    info!(
        studies = total_to_process,
        concurrency = config.concurrency,
        bars = config.num_bars,
        timeout = ?config.request_timeout,
        "Starting concurrent fundamental studies retrieval"
    );

    // 4. Concurrent execution with bounded semaphore
    let client = Arc::new(StudyClient::new(config.auth_token, config.server));
    let semaphore = Arc::new(Semaphore::new(config.concurrency));
    let mut join_set = JoinSet::new();

    for (idx, entry) in entries_to_process.into_iter().enumerate() {
        let sem = Arc::clone(&semaphore);
        let cli = Arc::clone(&client);
        let sym = sym_ticker.clone();
        let ex = effective_ex.clone();
        let num_bars = config.num_bars;
        let timeout = config.request_timeout;

        join_set.spawn(async move {
            let permit = match sem.acquire().await {
                Ok(p) => p,
                Err(_) => {
                    return (
                        idx,
                        entry,
                        FundamentalEntryStatus::Error("concurrency semaphore closed".to_string()),
                    );
                }
            };

            debug!(
                idx = idx,
                fund_id = %entry.fund_id,
                script_id = %entry.script_id,
                "fetching fundamental study"
            );

            let status = fetch_single_study(&cli, &entry, &sym, &ex, num_bars, timeout).await;
            drop(permit);

            (idx, entry, status)
        });
    }

    // 5. Drain completed tasks
    let mut raw_results = Vec::with_capacity(total_to_process);
    let mut processed_count = 0;

    while let Some(res) = join_set.join_next().await {
        match res {
            Ok((idx, entry, status)) => {
                processed_count += 1;
                if processed_count % 50 == 0 || processed_count == total_to_process {
                    debug!(
                        progress = format!("{processed_count}/{total_to_process}"),
                        pct = format!(
                            "{:.1}%",
                            (processed_count as f64 / total_to_process as f64) * 100.0
                        ),
                        "Study evaluation progress"
                    );
                }
                raw_results.push((idx, entry, status));
            }
            Err(err) => {
                return Err(Error::Internal(Ustr::from(&format!(
                    "background study fetch task failed: {err}"
                ))));
            }
        }
    }

    // 6. Restore deterministic canonical catalog order
    raw_results.sort_by_key(|(idx, _, _)| *idx);

    let entries = raw_results
        .into_iter()
        .map(|(_, entry, status)| FullFundamentalEntryResult { entry, status })
        .collect();

    let elapsed = start_total.elapsed();
    info!(
        total_time = ?elapsed,
        evaluated = total_to_process,
        "Completed full fundamental studies evaluation"
    );

    Ok(FullFundamentalResult {
        symbol,
        registry_date,
        schema_version,
        total_catalog_size,
        entries,
        elapsed,
    })
}

/// Executes a single fundamental Pine study retrieval for an entry.
async fn fetch_single_study(
    client: &StudyClient,
    entry: &FundamentalRegistryEntry,
    symbol: &str,
    exchange: &str,
    num_bars: u64,
    timeout: Duration,
) -> FundamentalEntryStatus {
    // 1. Fetch indicator metadata using exact stored script ID and version.
    let indicator = match entry.fetch_indicator(None).await {
        Ok(ind) => ind,
        Err(err) => {
            return FundamentalEntryStatus::Error(format!(
                "indicator metadata fetch failed: {err}"
            ));
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
                FundamentalEntryStatus::Empty
            } else {
                let mut points = result.data;
                // Sort points deterministically by (timestamp, index)
                points.sort_by(|a, b| {
                    let ts_a = a.value.first().copied().map(|v| v as i64).unwrap_or(0);
                    let ts_b = b.value.first().copied().map(|v| v as i64).unwrap_or(0);
                    ts_a.cmp(&ts_b).then_with(|| a.index.cmp(&b.index))
                });
                FundamentalEntryStatus::Success(points)
            }
        }
        Err(err) => FundamentalEntryStatus::Error(format!("study retrieval failed: {err}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use ustr::Ustr;

    fn make_test_entry(
        fund_id: &str,
        script_id: &str,
        script_name: &str,
        category: Option<&str>,
        period: Option<crate::models::FinancialPeriod>,
    ) -> FundamentalRegistryEntry {
        FundamentalRegistryEntry {
            fund_id: Ustr::from(fund_id),
            script_id: Ustr::from(script_id),
            script_version: Ustr::from("1.0"),
            script_name: Ustr::from(script_name),
            financial_period: period,
            fundamental_category: category.map(Ustr::from),
            short_description: None,
        }
    }

    #[test]
    fn test_parse_canonical_symbol() {
        assert_eq!(
            parse_canonical_symbol("NASDAQ:AAPL"),
            Some(("NASDAQ".to_string(), "AAPL".to_string()))
        );
        assert_eq!(
            parse_canonical_symbol("hose:fpt"),
            Some(("HOSE".to_string(), "FPT".to_string()))
        );
        assert_eq!(
            parse_canonical_symbol("  TWSE : 2330  "),
            Some(("TWSE".to_string(), "2330".to_string()))
        );

        // Non-canonical / bare ticker inputs
        assert_eq!(parse_canonical_symbol("AAPL"), None);
        assert_eq!(parse_canonical_symbol("2330"), None);
        assert_eq!(parse_canonical_symbol(""), None);
        assert_eq!(parse_canonical_symbol(":AAPL"), None);
        assert_eq!(parse_canonical_symbol("NASDAQ:"), None);
        assert_eq!(parse_canonical_symbol("A:B:C"), None);
    }

    #[test]
    fn test_is_stock_fundamental_study() {
        let stock_entry = make_test_entry(
            "total_revenue_fy",
            "STD;Fund_total_revenue_fy",
            "Total Revenue",
            Some("income_statement"),
            None,
        );
        assert!(is_stock_fundamental_study(&stock_entry));

        let crypto_entry = make_test_entry(
            "hashrate",
            "STD;CryptoFund_hashrate",
            "Hashrate",
            Some("blockchain"),
            None,
        );
        assert!(!is_stock_fundamental_study(&crypto_entry));
    }

    #[test]
    fn test_resolve_stock_from_candidates_success() {
        let fixture = vec![
            Symbol {
                symbol: "AAPL".to_string(),
                exchange: "NASDAQ".to_string(),
                market_type: "stock".to_string(),
                type_specs: vec!["common".to_string()],
                ..Default::default()
            },
            Symbol {
                symbol: "AAPL".to_string(),
                exchange: "BMV".to_string(),
                market_type: "stock".to_string(),
                type_specs: vec!["common".to_string()],
                ..Default::default()
            },
            Symbol {
                symbol: "AAPL".to_string(),
                exchange: "XETR".to_string(),
                market_type: "stock".to_string(),
                type_specs: vec!["common".to_string()],
                ..Default::default()
            },
        ];

        // Should choose NASDAQ because it is the first rank with market_type == stock and type_specs contains common
        let resolved = resolve_stock_from_candidates("AAPL", &fixture).unwrap();
        assert_eq!(resolved.exchange, "NASDAQ");
        assert_eq!(resolved.symbol, "AAPL");

        // Case insensitivity
        let resolved_lower = resolve_stock_from_candidates("aapl", &fixture).unwrap();
        assert_eq!(resolved_lower.exchange, "NASDAQ");
    }

    #[test]
    fn test_resolve_stock_from_candidates_not_found() {
        let fixture = vec![Symbol {
            symbol: "AAPL".to_string(),
            exchange: "NASDAQ".to_string(),
            market_type: "stock".to_string(),
            type_specs: vec!["common".to_string()],
            ..Default::default()
        }];

        let err = resolve_stock_from_candidates("MSFT", &fixture).unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn test_resolve_stock_from_candidates_ambiguity() {
        // Multiple matches, neither is common stock
        let fixture = vec![
            Symbol {
                symbol: "ABC".to_string(),
                exchange: "EX1".to_string(),
                market_type: "stock".to_string(),
                type_specs: vec!["preferred".to_string()],
                ..Default::default()
            },
            Symbol {
                symbol: "ABC".to_string(),
                exchange: "EX2".to_string(),
                market_type: "stock".to_string(),
                type_specs: vec!["warrant".to_string()],
                ..Default::default()
            },
        ];

        let err = resolve_stock_from_candidates("ABC", &fixture).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("ambiguous stock symbol 'ABC'"));
        assert!(msg.contains("EX1:ABC"));
        assert!(msg.contains("EX2:ABC"));
    }

    #[test]
    fn test_full_fundamental_result_counts_and_csv() {
        let entry1 = make_test_entry(
            "total_revenue_fy",
            "STD;Fund_total_revenue_fy",
            "Total Revenue",
            Some("income_statement"),
            Some(crate::models::FinancialPeriod::FiscalYear),
        );
        let entry2 = make_test_entry(
            "ebitda_fq",
            "STD;Fund_ebitda_fq",
            "EBITDA",
            Some("income_statement"),
            Some(crate::models::FinancialPeriod::FiscalQuarter),
        );
        let entry3 = make_test_entry("broken_metric", "STD;Fund_broken", "Broken", None, None);

        let symbol = Symbol {
            symbol: "AAPL".to_string(),
            exchange: "NASDAQ".to_string(),
            prefix: String::new(),
            ..Default::default()
        };

        let dp1 = DataPoint {
            index: 100,
            value: vec![1700000000.0, 42.5, 99.0],
        };
        let dp2 = DataPoint {
            index: 101,
            value: vec![1700086400.0, 45.0, 101.0],
        };

        let res = FullFundamentalResult {
            symbol,
            registry_date: NaiveDate::from_ymd_opt(2026, 9, 9).unwrap(),
            schema_version: 1,
            total_catalog_size: 1446,
            entries: vec![
                FullFundamentalEntryResult {
                    entry: entry1,
                    status: FundamentalEntryStatus::Success(vec![dp1, dp2]),
                },
                FullFundamentalEntryResult {
                    entry: entry2,
                    status: FundamentalEntryStatus::Empty,
                },
                FullFundamentalEntryResult {
                    entry: entry3,
                    status: FundamentalEntryStatus::Error("timeout reaching study".to_string()),
                },
            ],
            elapsed: Duration::from_millis(500),
        };

        assert_eq!(res.total_studies(), 3);
        assert_eq!(res.success_count(), 1);
        assert_eq!(res.empty_count(), 1);
        assert_eq!(res.error_count(), 1);
        assert_eq!(res.total_data_points(), 2);
        // Entry 1 has 2 points * 2 plots each = 4 rows
        // Entry 2 has 1 row (empty)
        // Entry 3 has 1 row (error)
        // Total rows = 6
        assert_eq!(res.total_csv_rows(), 6);

        let csv = res.to_csv_string().unwrap();
        let lines: Vec<&str> = csv.lines().collect();
        // 1 header + 6 data rows = 7 lines
        assert_eq!(lines.len(), 7);

        assert_eq!(
            lines[0],
            "registry_date,schema_version,fund_id,script_name,category,financial_period,script_id,script_version,symbol,exchange,status,error,timestamp,index,plot_index,value"
        );

        // First plot of dp1
        assert_eq!(
            lines[1],
            "2026-09-09,1,total_revenue_fy,Total Revenue,income_statement,FY,STD;Fund_total_revenue_fy,1.0,AAPL,NASDAQ,success,,1700000000,100,0,42.5"
        );
        // Second plot of dp1
        assert_eq!(
            lines[2],
            "2026-09-09,1,total_revenue_fy,Total Revenue,income_statement,FY,STD;Fund_total_revenue_fy,1.0,AAPL,NASDAQ,success,,1700000000,100,1,99"
        );

        // Empty study
        assert_eq!(
            lines[5],
            "2026-09-09,1,ebitda_fq,EBITDA,income_statement,FQ,STD;Fund_ebitda_fq,1.0,AAPL,NASDAQ,empty,,,,,"
        );

        // Errored study
        assert_eq!(
            lines[6],
            "2026-09-09,1,broken_metric,Broken,,,STD;Fund_broken,1.0,AAPL,NASDAQ,error,timeout reaching study,,,,"
        );
    }

    #[test]
    fn test_csv_record_escaping_and_roundtrip() {
        let mut buf = Vec::new();
        let fields = &[
            "simple",
            "with, comma",
            "with \"quotes\" inside",
            "multi\nline\r\nvalue",
            "Tiếng Việt: Lợi nhuận ròng",
            "中文: 净利润",
            "🚀 emoji",
        ];

        write_csv_record(&mut buf, fields).unwrap();

        // Read back with csv::Reader to ensure full RFC 4180 compatibility
        let mut rdr = csv::ReaderBuilder::new()
            .has_headers(false)
            .from_reader(&buf[..]);

        let mut records = rdr.records();
        let record = records.next().unwrap().unwrap();

        assert_eq!(record.len(), fields.len());
        for (i, expected) in fields.iter().enumerate() {
            assert_eq!(&record[i], *expected);
        }
    }
}
