//! Event data structures for the generic event pipeline.
//!
//! Every event type carries at minimum a `timestamp` (UTC seconds since epoch)
//! and a `symbol` identifier so that consumers can route events by instrument.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ustr::Ustr;

// ---------------------------------------------------------------------------
// Core event enum
// ---------------------------------------------------------------------------

/// The universal market event type.
///
/// All data flowing through the loader is represented as a variant of this enum.
/// It is `#[non_exhaustive]` so that new data kinds (e.g. options chains,
/// tick-level data) can be added without a breaking semver change.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MarketEvent {
    /// OHLCV / candlestick bar.
    Candle(CandleData),

    /// Real-time quote snapshot (bid, ask, last price, etc.).
    Quote(QuoteData),

    /// An economic indicator (CPI, GDP, unemployment, etc.).
    Economic(EconomicData),

    /// A single financial metric for a symbol (P/E, market cap, EPS, etc.).
    FinancialMetric(FinancialMetric),

    /// A corporate action announcement (dividend, split, merger, etc.).
    CorporateAction(CorporateAction),

    /// A news headline or article associated with a symbol.
    News(NewsEvent),

    /// Symbol metadata resolved by the data source (name, exchange, sector).
    SymbolResolved(SymbolInfoEvent),
}

impl MarketEvent {
    /// Return the event's timestamp in seconds since Unix epoch.
    pub fn timestamp(&self) -> i64 {
        match self {
            MarketEvent::Candle(c) => c.timestamp,
            MarketEvent::Quote(q) => q.timestamp,
            MarketEvent::Economic(e) => e.timestamp,
            MarketEvent::FinancialMetric(m) => m.timestamp,
            MarketEvent::CorporateAction(a) => a.timestamp,
            MarketEvent::News(n) => n.timestamp,
            MarketEvent::SymbolResolved(_) => 0,
        }
    }

    /// Return the symbol (instrument identifier) this event pertains to.
    pub fn symbol(&self) -> Option<&str> {
        match self {
            MarketEvent::Candle(c) => Some(c.symbol.as_str()),
            MarketEvent::Quote(q) => Some(q.symbol.as_str()),
            MarketEvent::Economic(_) => None,
            MarketEvent::FinancialMetric(m) => Some(m.symbol.as_str()),
            MarketEvent::CorporateAction(a) => Some(a.symbol.as_str()),
            MarketEvent::News(n) => n.symbol.as_deref(),
            MarketEvent::SymbolResolved(s) => Some(s.symbol.as_str()),
        }
    }
}

// ---------------------------------------------------------------------------
// Data kind enum — used by subscriptions
// ---------------------------------------------------------------------------

/// What kind of data a subscription is requesting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DataKind {
    /// Candlestick / OHLCV bars at a given interval.
    Candle,
    /// Real-time quote snapshots.
    Quote,
    /// Economic indicators.
    Economic,
    /// Financial statement metrics.
    FinancialMetric,
    /// Corporate actions.
    CorporateAction,
    /// News headlines and articles.
    News,
    /// Symbol metadata resolution.
    SymbolResolved,
}

// ---------------------------------------------------------------------------
// Individual event payloads
// ---------------------------------------------------------------------------

/// An OHLCV candlestick bar.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CandleData {
    /// Seconds since Unix epoch.
    pub timestamp: i64,
    /// Instrument identifier (e.g. `"BINANCE:BTCUSDT"`).
    pub symbol: Ustr,
    /// Bar interval (e.g. `"1D"`, `"1h"`).
    pub interval: Ustr,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

impl CandleData {
    /// Create a new candle from raw values.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        timestamp: i64,
        symbol: impl Into<Ustr>,
        interval: impl Into<Ustr>,
        open: f64,
        high: f64,
        low: f64,
        close: f64,
        volume: f64,
    ) -> Self {
        Self {
            timestamp,
            symbol: symbol.into(),
            interval: interval.into(),
            open,
            high,
            low,
            close,
            volume,
        }
    }

    /// Return the datetime for this candle.
    pub fn datetime(&self) -> Option<DateTime<Utc>> {
        DateTime::from_timestamp(self.timestamp, 0)
    }

    /// True if close > open.
    pub fn is_bullish(&self) -> bool {
        self.close > self.open
    }

    /// True if close < open.
    pub fn is_bearish(&self) -> bool {
        self.close < self.open
    }

    /// Typical price (H+L+C)/3.
    pub fn typical_price(&self) -> f64 {
        (self.high + self.low + self.close) / 3.0
    }
}

/// Real-time market quote.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuoteData {
    /// Seconds since Unix epoch (from the exchange).
    pub timestamp: i64,
    pub symbol: Ustr,
    pub bid: Option<f64>,
    pub ask: Option<f64>,
    pub bid_size: Option<f64>,
    pub ask_size: Option<f64>,
    pub last_price: Option<f64>,
    pub volume: Option<f64>,
    pub change: Option<f64>,
    pub change_percent: Option<f64>,
    pub open: Option<f64>,
    pub high: Option<f64>,
    pub low: Option<f64>,
    pub prev_close: Option<f64>,
}

impl QuoteData {
    /// Midpoint between bid and ask, if both are present.
    pub fn mid_price(&self) -> Option<f64> {
        match (self.bid, self.ask) {
            (Some(b), Some(a)) => Some((b + a) * 0.5),
            _ => None,
        }
    }

    /// Spread in absolute terms.
    pub fn spread(&self) -> Option<f64> {
        match (self.bid, self.ask) {
            (Some(b), Some(a)) => Some(a - b),
            _ => None,
        }
    }
}

/// An economic indicator datapoint (CPI, GDP, PMI, unemployment, etc.).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EconomicData {
    /// Seconds since Unix epoch of the observation date.
    pub timestamp: i64,
    /// Unique identifier for the indicator (e.g. `"US_CPI_YOY"`).
    pub indicator_id: Ustr,
    /// Human-readable name (e.g. `"US Consumer Price Index YoY"`).
    pub indicator_name: Ustr,
    /// Country or region code (ISO 3166-1 alpha-2).
    pub country: Ustr,
    /// The observed value.
    pub value: f64,
    /// Units (e.g. `"%"`, `"USD"`, `"pts"`).
    pub unit: Ustr,
    /// Frequency of observation.
    pub frequency: Ustr,
}

/// A financial metric associated with a security.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FinancialMetric {
    /// Seconds since Unix epoch.
    pub timestamp: i64,
    pub symbol: Ustr,
    pub metric_name: Ustr,
    pub value: f64,
    /// Optional unit (e.g. `"%"`, `"x"`, `"USD"`).
    pub unit: Option<Ustr>,
    /// Financial period (e.g. `"TTM"`, `"FY"`, `"FQ"`).
    pub period: Option<Ustr>,
}

/// A corporate action event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CorporateAction {
    /// Seconds since Unix epoch of the ex-date or announcement date.
    pub timestamp: i64,
    pub symbol: Ustr,
    pub action_type: CorporateActionType,
    /// Human-readable description.
    pub description: Ustr,
}

/// Types of corporate actions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CorporateActionType {
    Dividend,
    StockSplit,
    ReverseSplit,
    Merger,
    Acquisition,
    SpinOff,
    RightsOffering,
    Delisting,
    Ipo,
    Other,
}

impl std::fmt::Display for CorporateActionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CorporateActionType::Dividend => write!(f, "dividend"),
            CorporateActionType::StockSplit => write!(f, "stock_split"),
            CorporateActionType::ReverseSplit => write!(f, "reverse_split"),
            CorporateActionType::Merger => write!(f, "merger"),
            CorporateActionType::Acquisition => write!(f, "acquisition"),
            CorporateActionType::SpinOff => write!(f, "spin_off"),
            CorporateActionType::RightsOffering => write!(f, "rights_offering"),
            CorporateActionType::Delisting => write!(f, "delisting"),
            CorporateActionType::Ipo => write!(f, "ipo"),
            CorporateActionType::Other => write!(f, "other"),
        }
    }
}

/// A news headline or article.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NewsEvent {
    /// Seconds since Unix epoch of publication.
    pub timestamp: i64,
    /// Unique story ID from the provider.
    pub story_id: Ustr,
    /// News headline.
    pub title: Ustr,
    /// Optional full article body (may be truncated or absent).
    pub body: Option<Ustr>,
    /// News provider or source.
    pub provider: Ustr,
    /// URL to the full article.
    pub url: Option<Ustr>,
    /// Related symbols mentioned in the article.
    pub symbol: Option<Ustr>,
    pub tags: Vec<Ustr>,
}

/// Symbol metadata resolved from the data source.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SymbolInfoEvent {
    /// Instrument identifier.
    pub symbol: Ustr,
    /// Human-readable name.
    pub name: Ustr,
    /// Exchange the symbol trades on.
    pub exchange: Ustr,
    /// Short description.
    pub description: Ustr,
    /// ISO 4217 currency code.
    pub currency: Ustr,
    /// Market type classification.
    pub market_type: Ustr,
    /// Sector classification.
    pub sector: Option<Ustr>,
    /// Industry classification.
    pub industry: Option<Ustr>,
}

// ---------------------------------------------------------------------------
// OHLCV compat trait — bridges old code to the new event model
// ---------------------------------------------------------------------------

/// Trait providing OHLCV accessors for types that represent candlestick data.
///
/// This trait is intentionally kept compatible with the existing `OHLCV` trait
/// in `chart::models` so that migration can happen incrementally.
pub trait CandleLike {
    fn timestamp(&self) -> i64;
    fn open(&self) -> f64;
    fn high(&self) -> f64;
    fn low(&self) -> f64;
    fn close(&self) -> f64;
    fn volume(&self) -> f64;
}

impl CandleLike for CandleData {
    fn timestamp(&self) -> i64 {
        self.timestamp
    }
    fn open(&self) -> f64 {
        self.open
    }
    fn high(&self) -> f64 {
        self.high
    }
    fn low(&self) -> f64 {
        self.low
    }
    fn close(&self) -> f64 {
        self.close
    }
    fn volume(&self) -> f64 {
        self.volume
    }
}
