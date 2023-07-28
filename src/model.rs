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
