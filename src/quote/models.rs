use serde::{ Deserialize, Serialize };

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
    #[serde(default, rename(deserialize = "lp_time"))]
    pub timestamp: Option<f64>,
    #[serde(default)]
    pub volume: Option<f64>,

    #[serde(default, rename(deserialize = "currency_id"))]
    pub currency: Option<String>,
    #[serde(default, rename(deserialize = "short_name"))]
    pub symbol: Option<String>,
    #[serde(default, rename(deserialize = "exchange"))]
    pub exchange: Option<String>,
    #[serde(default, rename(deserialize = "type"))]
    pub market_type: Option<String>,
}
