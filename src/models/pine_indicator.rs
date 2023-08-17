use std::collections::HashMap;

use serde::Deserialize;
use serde_json::Value;

use crate::{
    chart::study::{IndicatorInput, InputValue},
    client::mics::get_indicator_metadata,
    models::FinancialPeriod,
    prelude::*,
    user::User,
};

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
    #[serde(rename(deserialize = "scriptIdPart"))]
    pub script_id: String,
    pub script_access: String,
    #[serde(rename(deserialize = "version"))]
    pub script_version: String,
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

#[derive(Debug, Clone, Deserialize)]
pub struct SearchResponse {
    pub next: String,
    pub result: Vec<PineSearchResult>,
}

#[derive(Default, Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PineSearchResult {
    pub image_url: String,
    pub script_name: String,
    pub script_source: String,
    pub access: i64,
    pub script_id_part: String,
    pub version: String,
    pub extra: PineSearchExtra,
    pub agree_count: i64,
    pub author: PineSearchAuthor,
    pub weight: i64,
}

#[derive(Default, Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PineSearchExtra {
    pub kind: Option<String>,
    pub source_inputs_count: Option<i64>,
    #[serde(rename = "isMTFResolution")]
    pub is_mtf_resolution: Option<bool>,
}

#[derive(Default, Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PineSearchAuthor {
    pub id: i64,
    pub username: String,
    #[serde(rename = "is_broker")]
    pub is_broker: bool,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct PineMetadata {
    #[serde(rename(deserialize = "IL"))]
    pub il: String,
    #[serde(rename(deserialize = "ilTemplate"))]
    pub il_template: String,
    #[serde(rename(deserialize = "metaInfo"))]
    pub data: PineMetadataInfo,
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
    pub plots: Vec<Plot>,
    pub styles: HashMap<String, Value>,
    pub uses_private_lib: bool,
    pub warnings: Vec<Value>,
}

#[derive(Debug, Default, Clone, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Plot {
    pub id: String,
    pub plot_type: String,
    pub target: Option<String>,
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
    pub input_type: String,
}

#[derive(Debug, Default, Clone, PartialEq, Deserialize)]
pub enum PineType {
    #[default]
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

impl std::fmt::Display for PineType {
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
            PineType::Script => write!(f, "Script@tv-scripting-101!"),
            PineType::StrategyScript => write!(f, "StrategyScript@tv-scripting-101!"),
            PineType::VolumeBasicStudies => write!(f, "Volume@tv-basicstudies-144"),
            PineType::FixedBasicStudies => write!(f, "VbPFixed@tv-basicstudies-139!"),
            PineType::FixedVolumeByPrice => write!(f, "VbPFixed@tv-volumebyprice-53!"),
            PineType::SessionVolumeByPrice => write!(f, "VbPSessions@tv-volumebyprice-53"),
            PineType::SessionRoughVolumeByPrice => {
                write!(f, "VbPSessionsRough@tv-volumebyprice-53!")
            }
            PineType::SessionDetailedVolumeByPrice => {
                write!(f, "VbPSessionsDetailed@tv-volumebyprice-53!")
            }
            PineType::VisibleVolumeByPrice => write!(f, "VbPVisible@tv-volumebyprice-53"),
        }
    }
}

pub struct PineIndicator {
    pub pine_type: PineType,
    pub info: PineInfo,
    pub options: PineMetadata,
}

impl PineIndicator {
    pub fn set_study_input(&self) -> HashMap<String, IndicatorInput> {
        let mut inputs: HashMap<String, IndicatorInput> = HashMap::new();
        inputs.insert(
            "text".to_string(),
            IndicatorInput::String(self.options.il_template.clone()),
        );
        inputs.insert(
            "pineId".to_string(),
            IndicatorInput::String(self.info.script_id.clone()),
        );
        inputs.insert(
            "pineVersion".to_string(),
            IndicatorInput::String(self.info.script_version.clone()),
        );
        self.options.data.inputs.iter().for_each(|input| {
            inputs.insert(
                input.id.clone(),
                IndicatorInput::IndicatorInput(InputValue {
                    v: Value::from(input.defval.clone()),
                    f: Value::from(input.is_fake.clone()),
                    t: Value::from(input.input_type.clone()),
                }),
            );
        });
        inputs
    }

    pub async fn fetch_metadata(&mut self, user: Option<&User>) -> Result<()> {
        self.options =
            get_indicator_metadata(user, &self.info.script_id, &self.info.script_version).await?;
        Ok(())
    }
}
