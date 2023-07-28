use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Default, Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Quote {
    #[serde(rename = "n")]
    pub ticker: String,
    #[serde(rename = "s")]
    pub status: String,
    #[serde(rename = "v")]
    pub quote: QuoteValue,
}

#[derive(Default, Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct QuoteValue {
    #[serde(default, rename(deserialize = "lp"))]
    pub price: f64,
    #[serde(default)]
    pub ask: f64,
    #[serde(default)]
    pub bid: f64,
    #[serde(default, rename(deserialize = "ch"))]
    pub charge: f64,
    #[serde(default, rename(deserialize = "chp"))]
    pub change_percent: f64,
    #[serde(default, rename(deserialize = "lp_time"))]
    pub timestamps: i64,
    #[serde(default)]
    pub volume: f64,
}

// #[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
// #[serde(untagged)]
// pub enum QuotePayload {
//     QuoteData(String, Quote),
//     QuoteLoaded(String),
// }

// #[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
// pub struct QuoteSocketMessage {
//     pub m: String,
//     pub p: QuotePayload,
// }
