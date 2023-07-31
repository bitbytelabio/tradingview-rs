use std::collections::HashMap;

use serde::Deserialize;
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

#[derive(Debug, Clone, Default)]
pub struct SimpleTA {
    pub name: String,
    pub data: HashMap<String, HashMap<String, f64>>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SymbolSearch {
    #[serde(rename(deserialize = "symbols_remaining"))]
    pub remaining: u64,
    pub symbols: Vec<Symbol>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Symbol {
    pub symbol: String,
    pub description: String,
    #[serde(rename(deserialize = "type"))]
    pub market_type: String,
    pub exchange: String,
    pub currency_code: String,
    #[serde(rename(deserialize = "provider_id"))]
    pub data_provider: String,
    #[serde(rename(deserialize = "country"))]
    pub country_code: String,
}
