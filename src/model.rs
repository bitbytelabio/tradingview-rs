use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Default, Debug, Clone, PartialEq, Deserialize)]
pub struct Indicator {
    #[serde(rename = "scriptName")]
    pub name: String,

    #[serde(rename = "scriptIdPart")]
    pub id: String,

    pub version: String,

    #[serde(flatten, rename = "extra")]
    pub info: HashMap<String, Value>,
}
