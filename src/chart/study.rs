use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use ustr::Ustr;

use crate::pine_indicator::PineIndicator;

/// An input value for a Pine Script indicator.
///
/// Can be either a plain string or a structured [`InputValue`] with value,
/// format, and type fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum IndicatorInput {
    String(Ustr),
    IndicatorInput(InputValue),
}

/// Structured indicator input with value, format, and type metadata.
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

/// A study configuration for a chart session.
///
/// - `Builtin` — A TradingView built-in indicator identified by name with
///   key-value parameters.
/// - `Pine` — A user-created or community Pine Script indicator.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StudyConfiguration {
    Builtin(String, HashMap<String, String>),
    Pine(PineIndicator),
}
