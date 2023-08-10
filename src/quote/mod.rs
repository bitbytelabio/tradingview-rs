pub mod session;
use serde::{Deserialize, Serialize};

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
    QuotePayload(Box<QuotePayload>),
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct QuotePayload {
    #[serde(rename(deserialize = "n"))]
    pub name: String,
    #[serde(rename(deserialize = "s"))]
    pub status: String,
    #[serde(rename(deserialize = "v"))]
    pub value: QuoteValue,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
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
    pub price_change: Option<f64>,
    #[serde(default, rename(deserialize = "chp"))]
    pub price_change_percent: Option<f64>,
    #[serde(default)]
    pub spread: Option<f64>,
    #[serde(default)]
    pub open_price: Option<f64>,
    #[serde(default)]
    pub high_price: Option<f64>,
    #[serde(default)]
    pub low_price: Option<f64>,
    #[serde(default)]
    pub prev_close_price: Option<f64>,
    #[serde(default, rename(deserialize = "lp"))]
    pub price: Option<f64>,
    #[serde(default, rename(deserialize = "lp_time"))]
    pub price_time: Option<i64>,
    #[serde(default)]
    pub volume: Option<f64>,
}
