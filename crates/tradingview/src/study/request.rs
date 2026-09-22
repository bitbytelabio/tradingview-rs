//! Study request builder and configuration.

use std::time::Duration;

use bon::Builder;

use crate::Result;
use crate::chart::study::StudyConfiguration;
use crate::error::TradingViewError;
use crate::{Interval, MarketSymbol, Ticker};

/// Immutable request object for a study or fundamental data fetch.
#[derive(Debug, Clone, Builder)]
#[builder(on(String, into))]
pub struct StudyRequest {
    /// Optional ticker specifying symbol and exchange.
    pub ticker: Option<Ticker>,
    /// Optional symbol string (e.g., "AAPL").
    pub symbol: Option<String>,
    /// Optional exchange string (e.g., "NASDAQ").
    pub exchange: Option<String>,
    /// Chart interval / resolution for the study.
    #[builder(default = Interval::OneDay)]
    pub interval: Interval,
    /// Base bar count requested for the primary series (default 100).
    #[builder(default = 100)]
    pub base_bar_count: u64,
    /// The study configuration (Builtin or Pine Script).
    #[builder(into)]
    pub study: StudyConfiguration,
    /// Optional custom study ID. If not provided, a random unique ID is generated.
    pub study_id: Option<String>,
    /// Request timeout (default 30 seconds).
    #[builder(default = Duration::from_secs(30))]
    pub timeout: Duration,
}

impl StudyRequest {
    /// Resolves the symbol and exchange from ticker or explicit fields.
    pub fn resolve_symbol_exchange(&self) -> Result<(String, String)> {
        if let Some(ticker) = &self.ticker {
            return Ok((ticker.symbol().to_string(), ticker.exchange().to_string()));
        }
        match (&self.symbol, &self.exchange) {
            (Some(s), Some(e)) => Ok((s.clone(), e.clone())),
            (None, _) => Err(TradingViewError::MissingSymbol.into()),
            (_, None) => Err(TradingViewError::MissingExchange.into()),
        }
    }
}
