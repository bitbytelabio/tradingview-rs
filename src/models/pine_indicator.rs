use std::collections::HashMap;

use serde::{Deserialize, Deserializer};
use serde_json::Value;

use crate::models::FinancialPeriod;

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

/**
 * @typedef {Struct} PineInput
 * @property {string} name Input name
 * @property {string} inline Input inline name
 * @property {string} [id] Input internal ID
 * @property {string} [tooltip] Input tooltip
 * @property {'text' | 'source' | 'integer'
 *  | 'float' | 'resolution' | 'bool' | 'color' | 'usertype'
 * } type Input type
 * @property {string | number | boolean} value Input default value
 * @property {boolean} isHidden If the input is hidden
 * @property {boolean} isFake If the input is fake
 * @property {string[]} [options] Input options if the input is a select
 */

#[derive(Debug, Default, Clone, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PineInput {
    pub name: String,
    pub inline: String,
    pub id: String,
    pub defval: Value,
    pub is_hidden: bool,
    pub is_fake: bool,
    pub optional: bool,
    pub options: Vec<String>,
    pub tooltip: Option<String>,
    #[serde(rename(deserialize = "type"))]
    pub input_type: PineInputType,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub enum PineInputType {
    #[default]
    Text,
    Sources,
    Integer,
    Float,
    Resolution,
    Bool,
    Color,
    UserType,
    Unknown,
}

impl<'de> Deserialize<'de> for PineInputType {
    fn deserialize<D>(deserializer: D) -> Result<PineInputType, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: String = Deserialize::deserialize(deserializer)?;
        match s.as_str() {
            "text" => Ok(PineInputType::Text),
            "source" => Ok(PineInputType::Sources),
            "integer" => Ok(PineInputType::Integer),
            "float" => Ok(PineInputType::Float),
            "resolution" => Ok(PineInputType::Resolution),
            "bool" => Ok(PineInputType::Bool),
            "color" => Ok(PineInputType::Color),
            "usertype" => Ok(PineInputType::UserType),
            _ => Ok(PineInputType::Unknown),
        }
    }
}

pub enum IndicatorType {
    Script,
    StrategyScript,
    VolumeBasicStudies,
    FixedBasicStudies,
    FixedVolumeByPrice,
    SessionVolumeByPrice,
    SessionRoughVolumeByPrice,
    SessionDetailedVolumeByPrice,
    VisibleVolumeByPrice,
}

impl std::fmt::Display for IndicatorType {
    /**
     * @typedef {'Script@tv-scripting-101!'
     *  | 'StrategyScript@tv-scripting-101!'} IndicatorType Indicator type
     * @typedef {'Volume@tv-basicstudies-144'
     *  | 'VbPFixed@tv-basicstudies-139!'
     *  | 'VbPFixed@tv-volumebyprice-53!'
     *  | 'VbPSessions@tv-volumebyprice-53'
     *  | 'VbPSessionsRough@tv-volumebyprice-53!'
     *  | 'VbPSessionsDetailed@tv-volumebyprice-53!'
     *  | 'VbPVisible@tv-volumebyprice-53'} BuiltInIndicatorType Built-in indicator type
     */
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            IndicatorType::Script => write!(f, "Script@tv-scripting-101!"),
            IndicatorType::StrategyScript => write!(f, "StrategyScript@tv-scripting-101!"),
            IndicatorType::VolumeBasicStudies => write!(f, "Volume@tv-basicstudies-144"),
            IndicatorType::FixedBasicStudies => write!(f, "VbPFixed@tv-basicstudies-139!"),
            IndicatorType::FixedVolumeByPrice => write!(f, "VbPFixed@tv-volumebyprice-53!"),
            IndicatorType::SessionVolumeByPrice => write!(f, "VbPSessions@tv-volumebyprice-53"),
            IndicatorType::SessionRoughVolumeByPrice => {
                write!(f, "VbPSessionsRough@tv-volumebyprice-53!")
            }
            IndicatorType::SessionDetailedVolumeByPrice => {
                write!(f, "VbPSessionsDetailed@tv-volumebyprice-53!")
            }
            IndicatorType::VisibleVolumeByPrice => write!(f, "VbPVisible@tv-volumebyprice-53"),
        }
    }
}

pub struct PineOptions {}

pub struct PineIndicator {
    /** {string} Indicator ID */
    pub pine_id: String,
    /** {string} Indicator version */
    pub pine_version: String,
    /** {string} Indicator description */
    pub description: String,
    /** {string} Indicator short description */
    pub short_description: String,
    /** {IndicatorInput} Indicator inputs */
    pub inputs: PineInput,
    /** {Object<string, string>} Indicator plots */
    pub plots: HashMap<String, String>,
    /** {IndicatorType} Indicator script */
    pub indicator_type: IndicatorType,
}

impl PineIndicator {}
