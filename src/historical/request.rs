use crate::chart::options::Range;
use crate::{Interval, MarketSymbol, Ticker};
use bon::Builder;

/// Immutable request object for a historical data fetch.
#[derive(Debug, Clone, Builder)]
pub struct HistoricalRequest {
    pub ticker: Option<Ticker>,
    pub symbol: Option<String>,
    pub exchange: Option<String>,
    #[builder(default = Interval::OneDay)]
    pub interval: Interval,
    pub range: Option<Range>,
    pub num_bars: Option<u64>,
    #[builder(default = false)]
    pub with_replay: bool,
    #[builder(default = std::time::Duration::from_secs(30))]
    pub timeout: std::time::Duration,
}

impl HistoricalRequest {
    pub fn resolve_symbol_exchange(&self) -> crate::Result<(String, String)> {
        if let Some(ref ticker) = self.ticker {
            return Ok((ticker.symbol().to_string(), ticker.exchange().to_string()));
        }
        match (&self.symbol, &self.exchange) {
            (Some(s), Some(e)) => Ok((s.clone(), e.clone())),
            (None, _) => Err(crate::error::TradingViewError::MissingSymbol.into()),
            (_, None) => Err(crate::error::TradingViewError::MissingExchange.into()),
        }
    }
}
