use serde::{Deserialize, Serialize};
use ustr::Ustr;

/// A single quote field as delivered by TradingView's WebSocket stream.
///
/// Each quote update contains a field `name` (e.g. `"lp"`, `"bid"`, `"ask"`),
/// a `status` code, and a [`QuoteValue`] payload.
#[derive(Debug, Default, Clone, PartialEq, Deserialize, Serialize, Copy)]
pub struct QuoteData {
    #[serde(rename(deserialize = "n"))]
    pub name: Ustr,
    #[serde(rename(deserialize = "s"))]
    pub status: Ustr,
    #[serde(rename(deserialize = "v"))]
    pub value: QuoteValue,
}

/// The value payload of a quote update.
///
/// All fields are optional — TradingView only includes the fields that have
/// changed since the last update. Common fields include `price` (last price),
/// `bid`/`ask`, `volume`, `change`/`change_percent`, and OHLC values.
#[derive(Clone, PartialEq, Deserialize, Serialize, Debug, Default, Copy)]
pub struct QuoteValue {
    #[serde(default)]
    pub ask: Option<f64>,
    #[serde(default)]
    pub ask_size: Option<f64>,
    #[serde(default)]
    pub bid: Option<f64>,
    #[serde(default)]
    pub bid_size: Option<f64>,
    #[serde(default, rename(deserialize = "ch"))]
    pub change: Option<f64>,
    #[serde(default, rename(deserialize = "chp"))]
    pub change_percent: Option<f64>,
    #[serde(default, rename(deserialize = "open_price"))]
    pub open: Option<f64>,
    #[serde(default, rename(deserialize = "high_price"))]
    pub high: Option<f64>,
    #[serde(default, rename(deserialize = "low_price"))]
    pub low: Option<f64>,
    #[serde(default, rename(deserialize = "prev_close_price"))]
    pub prev_close: Option<f64>,
    #[serde(default, rename(deserialize = "lp"))]
    pub price: Option<f64>,
    #[serde(default, rename(deserialize = "lp_time"))]
    pub timestamp: Option<f64>,
    #[serde(default)]
    pub volume: Option<f64>,
    #[serde(default, rename(deserialize = "currency_id"))]
    pub currency: Option<Ustr>,
    #[serde(default, rename(deserialize = "short_name"))]
    pub symbol: Option<Ustr>,
    #[serde(default, rename(deserialize = "exchange"))]
    pub exchange: Option<Ustr>,
    #[serde(default, rename(deserialize = "type"))]
    pub market_type: Option<Ustr>,
}
