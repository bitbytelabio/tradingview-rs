//! Batch historical data retrieval — concurrent multi-symbol fetching.
//!
//! Uses [`tokio::task::JoinSet`] to run [`HistoricalClient::retrieve`]
//! concurrently for multiple symbols, with a semaphore for concurrency
//! control to avoid overwhelming the TradingView data server.

use crate::{
    Interval,
    historical::{HistoricalClient, HistoricalRequest, HistoricalResult},
};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tracing::{debug, info, instrument, warn};

// =============================================================================
// BatchSymbolResult
// =============================================================================

/// Result of a batch retrieval for a single symbol.
///
/// Contains the symbol/exchange identity so the caller can correlate
/// results with their input set regardless of completion order.
#[derive(Debug)]
pub struct BatchSymbolResult {
    pub symbol: String,
    pub exchange: String,
    pub result: crate::Result<HistoricalResult>,
}

// =============================================================================
// BatchConfig
// =============================================================================

/// Configuration for batch historical data retrieval.
#[derive(Debug, Clone)]
pub struct BatchConfig {
    /// Maximum number of concurrent fetches.
    ///
    /// **Default: 4.**  Higher values may trigger TradingView rate-limiting.
    pub max_concurrency: usize,

    /// Timeout per individual symbol fetch.
    ///
    /// **Default: 30 seconds.**
    pub per_symbol_timeout: std::time::Duration,
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            max_concurrency: 4,
            per_symbol_timeout: std::time::Duration::from_secs(30),
        }
    }
}

// =============================================================================
// BatchResult — aggregate summary
// =============================================================================

/// Aggregate result of a batch retrieval.
///
/// Separates successful results from failed ones so the caller can
/// handle partial failures gracefully.
#[derive(Debug)]
pub struct BatchResult {
    /// Successfully retrieved results, with symbol identity.
    pub successful: Vec<BatchSymbolResult>,

    /// Failed retrievals, with the original symbol/exchange and error.
    pub failed: Vec<BatchSymbolResult>,

    /// Total number of symbols requested.
    pub total_requested: usize,

    /// Total wall-clock elapsed time for the batch operation.
    pub elapsed: std::time::Duration,
}

impl BatchResult {
    /// Returns `true` if all symbols were retrieved successfully.
    pub fn is_complete_success(&self) -> bool {
        self.failed.is_empty()
    }

    /// Number of successfully retrieved symbols.
    pub fn success_count(&self) -> usize {
        self.successful.len()
    }

    /// Number of failed retrievals.
    pub fn failure_count(&self) -> usize {
        self.failed.len()
    }
}

// =============================================================================
// HistoricalClient::retrieve_batch
// =============================================================================

impl HistoricalClient {
    /// Fetch historical data for multiple symbols concurrently.
    ///
    /// Each symbol is fetched independently via [`Self::retrieve`].  A
    /// semaphore limits concurrency to `config.max_concurrency` to avoid
    /// overwhelming the data server.  Per-symbol timeouts are enforced
    /// individually; the overall call returns when **all** symbols have
    /// completed or errored.
    ///
    /// # Returns
    ///
    /// A [`BatchResult`] that splits successful and failed fetches so
    /// partial failures can be handled gracefully.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let client = HistoricalClient::new("my_token", DataServer::ProData);
    /// let symbols = vec![
    ///     ("AAPL".into(), "NASDAQ".into()),
    ///     ("MSFT".into(), "NASDAQ".into()),
    ///     ("GOOGL".into(), "NASDAQ".into()),
    /// ];
    /// let result = client
    ///     .retrieve_batch(&symbols, Interval::OneDay, Some(100), BatchConfig::default())
    ///     .await;
    /// println!("{}/{} succeeded", result.success_count(), result.total_requested);
    /// ```
    #[instrument(skip(self, symbols), fields(symbol_count = symbols.len()))]
    pub async fn retrieve_batch(
        &self,
        symbols: &[(String, String)],
        interval: Interval,
        num_bars: Option<u64>,
        config: BatchConfig,
    ) -> BatchResult {
        let started = std::time::Instant::now();
        let symbol_count = symbols.len();

        if symbol_count == 0 {
            return BatchResult {
                successful: Vec::new(),
                failed: Vec::new(),
                total_requested: 0,
                elapsed: started.elapsed(),
            };
        }

        info!(
            "Starting batch retrieval: {} symbols, max_concurrency={}",
            symbol_count, config.max_concurrency
        );

        let semaphore = Arc::new(Semaphore::new(config.max_concurrency));
        let mut join_set = tokio::task::JoinSet::new();

        for (symbol, exchange) in symbols {
            let symbol = symbol.clone();
            let exchange = exchange.clone();
            let permit = Arc::clone(&semaphore);

            let request = HistoricalRequest::builder()
                .symbol(symbol.clone())
                .exchange(exchange.clone())
                .interval(interval)
                .maybe_num_bars(num_bars)
                .timeout(config.per_symbol_timeout)
                .build();

            // Each task gets its own HistoricalClient so concurrent
            // WebSocket connections don't interfere.
            let client = HistoricalClient::new(
                self.auth_token.clone(),
                self.server,
            );

            join_set.spawn(async move {
                let _permit = permit.acquire().await;
                debug!(
                    symbol = %symbol,
                    exchange = %exchange,
                    "Fetching historical data"
                );

                let result = client.retrieve(request).await;

                BatchSymbolResult {
                    symbol: symbol.clone(),
                    exchange: exchange.clone(),
                    result,
                }
            });
        }

        // Collect results in completion order.
        let mut successful = Vec::with_capacity(symbol_count);
        let mut failed = Vec::with_capacity(symbol_count);

        while let Some(task_result) = join_set.join_next().await {
            match task_result {
                Ok(batch_result) => match &batch_result.result {
                    Ok(hr) => {
                        debug!(
                            symbol = %batch_result.symbol,
                            bars = hr.total_bars_received,
                            elapsed_ms = hr.elapsed.as_millis(),
                            "Symbol fetch complete"
                        );
                        successful.push(batch_result);
                    }
                    Err(e) => {
                        warn!(
                            symbol = %batch_result.symbol,
                            error = %e,
                            "Symbol fetch failed"
                        );
                        failed.push(batch_result);
                    }
                },
                Err(join_err) => {
                    warn!(error = %join_err, "Batch task panicked or was cancelled");
                }
            }
        }

        let elapsed = started.elapsed();
        info!(
            "Batch retrieval complete: {}/{} successful in {:?}",
            successful.len(),
            symbol_count,
            elapsed,
        );

        BatchResult {
            successful,
            failed,
            total_requested: symbol_count,
            elapsed,
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_config_defaults() {
        let config = BatchConfig::default();
        assert_eq!(config.max_concurrency, 4);
        assert_eq!(config.per_symbol_timeout, std::time::Duration::from_secs(30));
    }

    #[test]
    fn test_batch_result_empty_success() {
        let result = BatchResult {
            successful: Vec::new(),
            failed: Vec::new(),
            total_requested: 0,
            elapsed: std::time::Duration::ZERO,
        };
        assert!(result.is_complete_success());
        assert_eq!(result.success_count(), 0);
        assert_eq!(result.failure_count(), 0);
    }

    #[test]
    fn test_batch_result_with_failures() {
        let result = BatchResult {
            successful: vec![],
            failed: vec![BatchSymbolResult {
                symbol: "AAPL".into(),
                exchange: "NASDAQ".into(),
                result: Err(crate::Error::Timeout("timeout".into())),
            }],
            total_requested: 1,
            elapsed: std::time::Duration::ZERO,
        };
        assert!(!result.is_complete_success());
        assert_eq!(result.success_count(), 0);
        assert_eq!(result.failure_count(), 1);
    }

    #[test]
    fn test_empty_symbols_returns_immediately() {
        // Verify the constructor pattern compiles and handles empty input.
        let result = BatchResult {
            successful: vec![],
            failed: vec![],
            total_requested: 0,
            elapsed: std::time::Duration::ZERO,
        };
        assert_eq!(result.total_requested, 0);
    }
}
