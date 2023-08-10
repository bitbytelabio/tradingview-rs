use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Indicator {
    pub info: IndicatorInfo,
    pub metadata: IndicatorMetadata,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct IndicatorInfo {
    #[serde(rename(deserialize = "scriptName"))]
    pub name: String,
    #[serde(rename(deserialize = "scriptIdPart"))]
    pub id: String,
    pub version: String,
    #[serde(flatten, rename(deserialize = "extra"))]
    pub info: HashMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct IndicatorMetadata {
    #[serde(rename(deserialize = "IL"))]
    pub il: String,
    #[serde(rename(deserialize = "ilTemplate"))]
    pub il_template: String,
    #[serde(rename(deserialize = "metaInfo"))]
    pub metainfo: IndicatorMetaInfo,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct IndicatorMetaInfo {
    pub id: String,
    #[serde(default, rename(deserialize = "_metainfoVersion"))]
    pub version: i32,
    #[serde(rename(deserialize = "scriptIdPart"))]
    pub pine_id: String,
    #[serde(default)]
    pub description: String,
    pub inputs: Vec<HashMap<String, Value>>,
    #[serde(default)]
    pub is_hidden_study: bool,
    #[serde(default)]
    pub is_price_study: bool,
    #[serde(rename(deserialize = "defaults"))]
    pub default: HashMap<String, Value>,
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
    #[serde(default)]
    pub description: String,
    #[serde(default, rename(deserialize = "type"))]
    pub market_type: String,
    #[serde(default)]
    pub exchange: String,
    #[serde(default)]
    pub currency_code: String,
    #[serde(default, rename(deserialize = "provider_id"))]
    pub data_provider: String,
    #[serde(default, rename(deserialize = "country"))]
    pub country_code: String,
}

#[derive(Debug, PartialEq, Clone)]
pub struct OHLCV {
    pub time: f64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}
