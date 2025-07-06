use serde::{Deserialize, Serialize};
use ustr::Ustr;

#[derive(Debug, Default, Clone, PartialEq, Deserialize, Serialize, Copy)]
pub struct QuoteData {
    #[serde(rename(deserialize = "n"))]
    pub name: Ustr,
    #[serde(rename(deserialize = "s"))]
    pub status: Ustr,
    #[serde(rename(deserialize = "v"))]
    pub value: QuoteValue,
}

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
