use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Indicator {
    #[serde(rename = "scriptName")]
    pub name: String,

    #[serde(rename = "scriptIdPart")]
    pub id: String,

    pub version: String,

    #[serde(flatten, rename = "extra")]
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
    #[serde(default, rename = "lp")]
    price: f64,
    #[serde(default)]
    ask: f64,
    #[serde(default)]
    bid: f64,
    #[serde(default, rename = "ch")]
    charge: f64,
    #[serde(default, rename = "lp_time")]
    timestamps: u64,
    #[serde(default)]
    volume: f64,
}
