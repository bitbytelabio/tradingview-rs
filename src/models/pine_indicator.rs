use std::collections::HashMap;

use serde::Deserialize;
use serde_json::Value;

use crate::{
    chart::study::{IndicatorInput, InputValue},
    client::mics::get_indicator_metadata,
    models::{FinancialPeriod, UserCookies},
    Result,
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
    pub script_id: String,
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
pub enum ScriptType {
    #[default]
    Script,
    IntervalScript,
    StrategyScript,
    VolumeBasicStudies,
    FixedBasicStudies,
    FixedVolumeByPrice,
    SessionVolumeByPrice,
    SessionRoughVolumeByPrice,
    SessionDetailedVolumeByPrice,
    VisibleVolumeByPrice,
}

impl std::fmt::Display for ScriptType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            ScriptType::Script => write!(f, "Script@tv-scripting-101!"),
            ScriptType::IntervalScript => write!(f, "Internal@tv-scripting-101!"),
            ScriptType::StrategyScript => write!(f, "StrategyScript@tv-scripting-101!"),
            ScriptType::VolumeBasicStudies => write!(f, "Volume@tv-basicstudies-144"),
            ScriptType::FixedBasicStudies => write!(f, "VbPFixed@tv-basicstudies-139!"),
            ScriptType::FixedVolumeByPrice => write!(f, "VbPFixed@tv-volumebyprice-53!"),
            ScriptType::SessionVolumeByPrice => write!(f, "VbPSessions@tv-volumebyprice-53"),
            ScriptType::SessionRoughVolumeByPrice => {
                write!(f, "VbPSessionsRough@tv-volumebyprice-53!")
            }
            ScriptType::SessionDetailedVolumeByPrice => {
                write!(f, "VbPSessionsDetailed@tv-volumebyprice-53!")
            }
            ScriptType::VisibleVolumeByPrice => write!(f, "VbPVisible@tv-volumebyprice-53"),
        }
    }
}

pub struct PineIndicator {
    pub script_id: String,
    pub script_version: String,
    pub script_type: ScriptType,
    pub metadata: PineMetadata,
}

pub struct PineIndicatorBuilder {
    user: Option<UserCookies>,
}

impl PineIndicatorBuilder {
    pub fn user(&mut self, user: UserCookies) -> &mut Self {
        self.user = Some(user);
        self
    }

    pub async fn fetch(
        &mut self,
        script_id: &str,
        script_version: &str,
        script_type: ScriptType,
    ) -> Result<PineIndicator> {
        let metadata = match &self.user {
            Some(user) => get_indicator_metadata(Some(user), script_id, script_version).await?,
            None => get_indicator_metadata(None, script_id, script_version).await?,
        };
        Ok(PineIndicator {
            script_id: script_id.to_string(),
            script_version: script_version.to_string(),
            script_type,
            metadata,
        })
    }
}

impl PineIndicator {
    pub fn build() -> PineIndicatorBuilder {
        PineIndicatorBuilder { user: None }
    }

    pub fn to_study_inputs(&self) -> Result<Value> {
        let mut inputs: HashMap<String, IndicatorInput> = HashMap::new();
        inputs.insert(
            "text".to_string(),
            IndicatorInput::String(self.metadata.il_template.clone()),
        );
        inputs.insert(
            "pineId".to_string(),
            IndicatorInput::String(self.script_id.clone()),
        );
        inputs.insert(
            "pineVersion".to_string(),
            IndicatorInput::String(self.script_version.clone()),
        );
        self.metadata.data.inputs.iter().for_each(|input| {
            if input.id == "text" || input.id == "pineId" || input.id == "pineVersion" {
                return;
            }
            inputs.insert(
                input.id.clone(),
                IndicatorInput::IndicatorInput(InputValue {
                    v: input.defval.clone(),
                    f: Value::from(input.is_fake),
                    t: Value::from(input.input_type.clone()),
                }),
            );
        });

        let json_value = serde_json::to_value(inputs)?;
        Ok(json_value)
    }
}
