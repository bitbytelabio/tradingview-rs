use std::{collections::HashMap, default};

use serde::{Deserialize, Deserializer};
use serde_json::Value;

use super::FinancialPeriod;

#[derive(Debug, Clone, PartialEq)]
pub enum BuiltinIndicators {
    All,
    Fundamental,
    Standard,
    Candlestick,
}

impl std::fmt::Display for BuiltinIndicators {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            BuiltinIndicators::All => write!(f, "all"),
            BuiltinIndicators::Fundamental => write!(f, "fundamental"),
            BuiltinIndicators::Standard => write!(f, "standard"),
            BuiltinIndicators::Candlestick => write!(f, "candlestick"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PineInfo {
    pub user_id: i64,
    pub script_name: String,
    pub script_source: String,
    pub script_id_part: String,
    pub script_access: String,
    pub version: String,
    pub extra: PineInfoExtra,
}

#[derive(Debug, Default, Clone, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PineInfoExtra {
    pub financial_period: Option<FinancialPeriod>,
    pub fund_id: Option<String>,
    pub fundamental_category: Option<String>,
    pub is_auto: bool,
    pub is_beta: bool,
    pub is_built_in: bool,
    pub is_candle_stick: bool,
    pub is_fundamental_study: bool,
    pub is_hidden_study: bool,
    pub is_mtf_resolution: bool,
    pub is_new: bool,
    pub is_pine_editor_new_template: bool,
    pub is_updated: bool,
    pub kind: String,
    pub short_description: String,
    pub source_inputs_count: i64,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TranslateResponse {
    pub success: bool,
    pub result: PineMetadata,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PineMetadata {
    #[serde(rename(deserialize = "IL"))]
    pub il: String,
    #[serde(rename(deserialize = "ilTemplate"))]
    pub il_template: String,
    #[serde(rename(deserialize = "metaInfo"))]
    pub meta_info: PineMetadataInfo,
}

#[derive(Debug, Default, Clone, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PineMetadataInfo {
    pub id: String,
    #[serde(rename(deserialize = "scriptIdPart"))]
    pub script_id: String,
    pub description: String,
    pub short_description: String,
    pub financial_period: Option<FinancialPeriod>,
    pub grouping_key: String,
    pub is_fundamental_study: bool,
    pub is_hidden_study: bool,
    pub is_tv_script: bool,
    pub is_tv_script_stub: bool,
    pub is_price_study: bool,
    pub inputs: Vec<PineInput>,
    pub defaults: HashMap<String, Value>,
    pub palettes: HashMap<String, Value>,
    pub pine: HashMap<String, String>,
    pub plots: Vec<HashMap<String, String>>,
    pub styles: HashMap<String, Value>,
    pub uses_private_lib: bool,
    pub warnings: Vec<Value>,
}

#[derive(Debug, Default, Clone, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PineInput {
    pub id: String,
    pub name: String,
    pub defval: Value,
    pub is_hidden: bool,
    pub is_fake: bool,
    pub optional: bool,
    pub options: Vec<String>,
    pub tooltip: Option<String>,
    #[serde(rename(deserialize = "type"))]
    pub input_type: String,
}
