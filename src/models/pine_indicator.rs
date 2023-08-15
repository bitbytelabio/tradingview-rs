use std::collections::HashMap;

use serde::Deserialize;
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
pub struct Info {
    pub user_id: i64,
    pub script_name: String,
    pub script_source: String,
    pub script_id_part: String,
    pub script_access: String,
    pub version: String,
    pub extra: Extra,
}

#[derive(Debug, Default, Clone, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Extra {
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
    pub result: Metadata,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Metadata {
    #[serde(rename(deserialize = "IL"))]
    pub il: String,
    #[serde(rename(deserialize = "ilTemplate"))]
    pub il_template: String,
    #[serde(rename(deserialize = "metaInfo"))]
    pub meta_info: MetadataInfo,
}

#[derive(Debug, Default, Clone, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MetadataInfo {
    pub id: String,
    pub script_id_part: String,
    pub description: String,
    pub short_description: String,
    pub financial_period: Option<FinancialPeriod>,
    pub is_fundamental_study: bool,
    pub is_hidden_study: bool,
    pub is_tv_script: bool,
    pub is_tv_script_stub: bool,
    pub is_price_study: bool,
    pub inputs: Vec<Input>,
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
pub struct Input {
    #[serde(rename(deserialize = "defval"))]
    pub def_val: String,
    pub id: String,
    pub name: String,
    pub is_hidden: bool,
    pub is_fake: bool,
    pub optional: bool,
    pub options: Vec<String>,
    #[serde(rename(deserialize = "type"))]
    pub format_type: String,
}

#[derive(Debug, Default, Clone, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct DefaultInput {
    pub pine_features: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
struct IndicatorInput {
    name: String,
    inline: String,
    internal_id: Option<String>,
    tooltip: Option<String>,
    input_type: InputType,
    value: InputValue,
    is_hidden: bool,
    is_fake: bool,
    options: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq)]
enum InputType {
    Text,
    Source,
    Integer,
    Float,
    Resolution,
    Bool,
    Color,
}
#[derive(Debug, Clone, PartialEq)]
enum InputValue {
    String(String),
    Number(f64),
    Boolean(bool),
}

#[derive(Debug, Clone, PartialEq)]
struct Indicator {
    pine_id: String,
    pine_version: String,
    description: String,
    short_description: String,
    inputs: HashMap<String, IndicatorInput>,
    plots: HashMap<String, String>,
    script: String,
}

#[derive(Debug, Clone, PartialEq)]
enum IndicatorType {
    Script,
    StrategyScript,
}

#[derive(Debug, Clone, PartialEq)]
struct PineIndicator {
    options: Indicator,
    indicator_type: IndicatorType,
}

impl PineIndicator {
    fn new(options: Indicator) -> Self {
        PineIndicator {
            options,
            indicator_type: IndicatorType::Script,
        }
    }

    fn pine_id(&self) -> &str {
        &self.options.pine_id
    }

    fn pine_version(&self) -> &str {
        &self.options.pine_version
    }

    fn description(&self) -> &str {
        &self.options.description
    }

    fn short_description(&self) -> &str {
        &self.options.short_description
    }

    fn inputs(&self) -> &HashMap<String, IndicatorInput> {
        &self.options.inputs
    }

    fn plots(&self) -> &HashMap<String, String> {
        &self.options.plots
    }

    fn indicator_type(&self) -> IndicatorType {
        self.indicator_type.clone()
    }

    fn set_indicator_type(&mut self, indicator_type: IndicatorType) {
        self.indicator_type = indicator_type;
    }

    fn script(&self) -> &str {
        &self.options.script
    }

    fn set_option(&mut self, key: &str, value: InputValue) -> Result<(), String> {
        let mut prop_i = String::new();

        if self.options.inputs.contains_key(&format!("in_{}", key)) {
            prop_i = format!("in_{}", key);
        } else if self.options.inputs.contains_key(key) {
            prop_i = key.to_string();
        } else {
            for (i, input) in self.options.inputs.iter() {
                if input.inline == key || input.internal_id == Some(key.to_string()) {
                    prop_i = i.to_string();
                    break;
                }
            }
        }

        if !prop_i.is_empty() && self.options.inputs.contains_key(&prop_i) {
            let input = self.options.inputs.get_mut(&prop_i).unwrap();

            let types = [
                (InputType::Bool, "Boolean"),
                (InputType::Integer, "Number"),
                (InputType::Float, "Number"),
                (InputType::Text, "String"),
            ];

            let input_type = input.input_type.clone();
            let value_type = match &value {
                InputValue::String(_) => "String",
                InputValue::Number(_) => "Number",
                InputValue::Boolean(_) => "Boolean",
            };

            if let Some(t) = types.iter().find(|(t, _)| *t == input_type) {
                if t.1 != value_type {
                    return Err(format!(
                        "Input '{}' ({}) must be a {} !",
                        input.name, prop_i, t.1
                    ));
                }
            }

            // if let Some(options) = &input.options {
            //     if !options.contains(&value.to_string()) {
            //         return Err(format!(
            //             "Input '{}' ({}) must be one of these values: {:?}",
            //             input.name, prop_i, options
            //         ));
            //     }
            // }

            input.value = value;
            Ok(())
        } else {
            Err(format!("Input '{}' not found ({})", key, prop_i))
        }
    }
}
