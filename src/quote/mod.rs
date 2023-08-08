pub mod websocket;
use serde::{Deserialize, Serialize};

use crate::error::TradingViewError;

lazy_static::lazy_static! {
    pub static ref ALL_QUOTE_FIELDS: Vec<&'static str> = vec![
        "lp",
        "lp_time",
        "ask",
        "bid",
        "bid_size",
        "ask_size",
        "ch",
        "chp",
        "volume",
        "high_price",
        "low_price",
        "open_price",
        "prev_close_price",
        "currency_code",
        "current_session",
        "description",
        "exchange",
        "format",
        "fractional",
        "is_tradable",
        "language",
        "local_description",
        "logoid",
        "minmov",
        "minmove2",
        "original_name",
        "pricescale",
        "pro_name",
        "short_name",
        "type",
        "update_mode",
        "fundamentals",
        "rch",
        "rchp",
        "rtc",
        "rtc_time",
        "status",
        "industry",
        "basic_eps_net_income",
        "beta_1_year",
        "market_cap_basic",
        "earnings_per_share_basic_ttm",
        "price_earnings_ttm",
        "sector",
        "dividends_yield",
        "timezone",
        "country_code",
        "provider_id",
    ];
}

#[derive(Debug)]
pub enum QuoteEvent {
    Data,
    Loaded,
    Error(TradingViewError),
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct QuoteSocketMessage {
    #[serde(rename = "m")]
    pub message_type: String,
    #[serde(rename = "p")]
    pub payload: Vec<QuotePayloadType>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum QuotePayloadType {
    String(String),
    QuotePayload(QuotePayload),
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct QuotePayload {
    #[serde(rename = "n")]
    pub name: String,
    #[serde(rename = "s")]
    pub status: String,
    #[serde(rename = "v")]
    pub value: QuoteValue,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct QuoteValue {
    #[serde(default)]
    pub ask: f64,
    #[serde(default)]
    pub ask_size: f64,
    #[serde(default)]
    pub bid: f64,
    #[serde(default)]
    pub bid_size: f64,
    #[serde(default, rename = "ch")]
    pub price_change: f64,
    #[serde(default, rename = "chp")]
    pub price_change_percent: f64,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub exchange: Option<String>,
    #[serde(default)]
    pub short_name: Option<String>,
    #[serde(default)]
    pub spread: f64,
    #[serde(default)]
    pub open_price: f64,
    #[serde(default)]
    pub high_price: f64,
    #[serde(default)]
    pub low_price: f64,
    #[serde(default)]
    pub prev_close_price: f64,
    #[serde(default, rename = "lp")]
    pub price: f64,
    #[serde(default, rename = "lp_time")]
    pub price_time: i64,
    #[serde(default)]
    pub volume: f64,
}
