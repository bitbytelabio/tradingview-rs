pub mod session;
use serde::{ Deserialize, Deserializer, Serialize };

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
        "currency_id",
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
        "country",
        "provider_id"
    ];
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct QuoteData {
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
    #[serde(default, rename(deserialize = "lp_time"), deserialize_with = "deserialize_timestamp")]
    pub timestamp: Option<i64>,
    #[serde(default)]
    pub volume: Option<f64>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub country: Option<String>,
    #[serde(default, rename(deserialize = "currency_id"))]
    pub currency: Option<String>,
    #[serde(default, rename(deserialize = "provider_id"))]
    pub data_provider: Option<String>,
    #[serde(default, rename(deserialize = "short_name"))]
    pub symbol: Option<String>,
    #[serde(default, rename(deserialize = "pro_name"))]
    pub symbol_id: Option<String>,
    #[serde(default, rename(deserialize = "exchange"))]
    pub exchange: Option<String>,
    #[serde(default, rename(deserialize = "type"))]
    pub market_type: Option<String>,
}

fn deserialize_timestamp<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
    where D: Deserializer<'de>
{
    let timestamp_seconds: Option<i64> = Option::deserialize(deserializer)?;
    Ok(timestamp_seconds.map(|s| s * 1000))
}
