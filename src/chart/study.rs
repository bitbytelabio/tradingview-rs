use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use ustr::Ustr;

use crate::pine_indicator::PineIndicator;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum IndicatorInput {
    String(Ustr),
    IndicatorInput(InputValue),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InputValue {
    pub v: Value,
    pub f: Value,
    pub t: Value,
}

impl InputValue {
    pub fn new(v: Value, f: Value, t: Value) -> InputValue {
        InputValue { v, f, t }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StudyConfiguration {
    Builtin(String, HashMap<String, String>),
    Pine(PineIndicator),
}
