//! Date-versioned registry of TradingView fundamental Pine studies.
//!
//! Fundamental financial metrics in TradingView (income statement, balance sheet,
//! cash flow, valuation statistics, etc.) are implemented as built-in Pine Script studies.
//!
//! This module provides:
//! - [`FundamentalRegistry`] — A date-versioned, deterministically ordered snapshot of
//!   fundamental studies with fast non-panicking lookup by exact or base `fund_id` and period.
//! - [`FundamentalRegistryEntry`] — A registered study with its canonical `fund_id`, `script_id`,
//!   `script_version`, reporting period, and conversion helpers.
//! - [`FundamentalRegistryFilter`] — Flexible search and filtering across categories, names,
//!   fund IDs, and periods.
//! - [`fetch_fundamental_registry`] — Network fetcher retrieving the live catalog from TradingView's
//!   Pine facade endpoint.
//!
//! # Architecture
//!
//! ```text
//! fetch_fundamental_registry()
//!   └── GET https://pine-facade.tradingview.com/pine-facade/list/?filter=fundamental
//!         ├── Filters extra.is_fundamental_study == true
//!         ├── Preserves exact scriptIdPart + version + UTC date tag
//!         └── Builds FundamentalRegistry (deterministic order + fast Ustr index)
//!
//! FundamentalRegistry
//!   ├── .get("total_revenue_fy")                                      // Exact lookup
//!   ├── .lookup("total_revenue", Some(&FinancialPeriod::FiscalYear))  // Base + Period
//!   ├── .filter(&filter)                                              // Multi-criteria filter
//!   ├── .save_to_file("registry_2026-09-09.json")                    // Offline persistence
//!   └── entry.to_study_options() / entry.fetch_indicator()            // Chart study wiring
//! ```
//!
//! # Example
//!
//! ```no_run
//! use tradingview::fundamental::{
//!     fetch_fundamental_registry, FundamentalRegistryFilter,
//! };
//! use tradingview::models::FinancialPeriod;
//!
//! # async fn run() -> tradingview::Result<()> {
//! // Fetch current fundamental studies catalog
//! let registry = fetch_fundamental_registry().await?;
//!
//! // Look up a specific metric by canonical fund ID:
//! if let Some(entry) = registry.get("total_revenue_fy") {
//!     println!("Found study: {} (script: {})", entry.script_name, entry.script_id);
//! }
//!
//! // Look up by base metric and reporting period:
//! if let Some(entry) = registry.lookup("total_revenue", Some(&FinancialPeriod::TrailingTwelveMonths)) {
//!     println!("TTM Revenue study ID: {}", entry.script_id);
//! }
//!
//! // Save locally for offline use:
//! registry.save_to_file("registry_2026-09-09.json")?;
//! # Ok(())
//! # }
//! ```

pub mod entry;
pub mod fetch;
pub mod filter;
pub mod full;
pub mod registry;
#[cfg(test)]
mod tests;

pub use entry::FundamentalRegistryEntry;
pub use fetch::{
    PINE_FACADE_FUNDAMENTAL_URL, fetch_fundamental_registry, fetch_fundamental_registry_for_date,
    fetch_fundamental_registry_with_client,
};
pub use filter::FundamentalRegistryFilter;
pub use full::{
    FullFundamentalConfig, FullFundamentalEntryResult, FullFundamentalResult,
    FundamentalEntryStatus, fetch_full_fundamentals, fetch_full_fundamentals_with_config,
};
pub use registry::FundamentalRegistry;

/// Retrieves fundamental data points for a symbol using a registered study or canonical identifier.
///
/// High-level one-shot workflow:
/// 1. Looks up the metric in the provided [`FundamentalRegistry`] (by exact or base identifier and optional period).
/// 2. Fetches indicator metadata (`ScriptType::IntervalScript`).
/// 3. Executes [`StudyClient::retrieve`](crate::study::StudyClient::retrieve) over WebSocket.
#[allow(clippy::too_many_arguments)]
pub async fn get_fundamental_data(
    registry: &FundamentalRegistry,
    fund_id: &str,
    period: Option<&crate::models::FinancialPeriod>,
    symbol: &str,
    exchange: &str,
    interval: crate::models::Interval,
    num_bars: u64,
    auth_token: Option<&str>,
    server: crate::live::models::DataServer,
) -> crate::Result<crate::study::StudyResult> {
    let entry = registry.lookup(fund_id, period).ok_or_else(|| {
        crate::Error::Internal(
            format!("fundamental study '{fund_id}' not found in registry").into(),
        )
    })?;

    let indicator = entry.fetch_indicator(None).await?;
    let client =
        crate::study::StudyClient::new(auth_token.unwrap_or("unauthorized_user_token"), server);

    let request = crate::study::StudyRequest::builder()
        .symbol(symbol)
        .exchange(exchange)
        .interval(interval)
        .base_bar_count(num_bars)
        .study(crate::chart::study::StudyConfiguration::Pine(indicator))
        .build();

    client.retrieve(request).await
}
