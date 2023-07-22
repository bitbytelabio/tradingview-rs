use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Indicator {
    #[serde(rename(deserialize = "scriptName"))]
    pub name: String,
    #[serde(rename(deserialize = "scriptIdPart"))]
    pub id: String,
    pub version: String,
    #[serde(flatten, rename(deserialize = "extra"))]
    pub info: HashMap<String, Value>,
}

// "p": Array [String("qs_lGjs9GgIRhdM"), Object {"n": String("BINANCE:BTCUSDT"), "s": String("ok"), "v": Object {"ask": Number(29884.37), "bid": Number(29884.36)}}]}
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Quote {
    #[serde(rename = "n")]
    pub ticker: String,
    #[serde(rename = "s")]
    pub status: String,
    #[serde(rename = "v")]
    pub quote: QuoteValue,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuoteValue {
    #[serde(default, rename(deserialize = "lp"))]
    pub price: f64,
    #[serde(default)]
    pub ask: f64,
    #[serde(default)]
    pub bid: f64,
    #[serde(default, rename(deserialize = "ch"))]
    pub charge: f64,
    #[serde(default, rename(deserialize = "lp_time"))]
    pub timestamps: u64,
    #[serde(default)]
    pub volume: f64,
}
