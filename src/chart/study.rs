use serde::Serialize;
use serde_json::Value;
use ustr::Ustr;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum IndicatorInput {
    String(Ustr),
    IndicatorInput(InputValue),
}

#[derive(Debug, Clone, PartialEq, Serialize)]
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
