use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Quote {
    #[serde(rename = "n")]
    pub ticker: String,
    #[serde(rename = "s")]
    pub status: String,
    #[serde(rename = "v")]
    pub quote: QuoteValue,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum QuoteValue {
    Metadata {
        #[serde(default, rename(deserialize = "lp"))]
        price: f64,
        #[serde(default)]
        ask: f64,
        #[serde(default)]
        bid: f64,
        #[serde(default, rename(deserialize = "ch"))]
        charge: f64,
        #[serde(default, rename(deserialize = "chp"))]
        change_percent: f64,
        #[serde(default, rename(deserialize = "lp_time"))]
        timestamps: i64,
        #[serde(default)]
        volume: f64,
        #[serde(default)]
        currency_code: String,
        #[serde(default)]
        description: String,
        #[serde(default)]
        exchange: String,
        #[serde(default, rename(deserialize = "type"))]
        market_type: String,
    },
    Data {
        #[serde(default, rename(deserialize = "lp"))]
        price: f64,
        #[serde(default)]
        ask: f64,
        #[serde(default)]
        bid: f64,
        #[serde(default, rename(deserialize = "ch"))]
        charge: f64,
        #[serde(default, rename(deserialize = "chp"))]
        change_percent: f64,
        #[serde(default, rename(deserialize = "lp_time"))]
        timestamps: i64,
        #[serde(default)]
        volume: f64,
    },
}
