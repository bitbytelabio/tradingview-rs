use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use ustr::Ustr;

use crate::{
    Result,
    chart::study::{IndicatorInput, InputValue},
    client::misc::get_indicator_metadata,
    models::{FinancialPeriod, UserCookies},
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
    pub script_name: Ustr,
    pub script_source: Ustr,
    #[serde(rename(deserialize = "scriptIdPart"))]
    pub script_id: Ustr,
    pub script_access: Ustr,
    #[serde(rename(deserialize = "version"))]
    pub script_version: Ustr,
    pub extra: PineInfoExtra,
}

#[derive(Debug, Default, Clone, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PineInfoExtra {
    pub financial_period: Option<FinancialPeriod>,
    #[serde(alias = "fund_id")]
    pub fund_id: Option<Ustr>,
    pub fundamental_category: Option<Ustr>,
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
    pub kind: Ustr,
    pub short_description: Ustr,
    pub source_inputs_count: i64,
    pub tags: Vec<Ustr>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TranslateResponse {
    pub success: bool,
    pub result: PineMetadata,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SearchResponse {
    pub next: String,
    pub results: Vec<PineSearchResult>,
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

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PineSearchAuthor {
    pub id: i64,
    pub username: Ustr,
    #[serde(rename = "is_broker")]
    pub is_broker: bool,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, Default)]
pub struct PineMetadata {
    /// No longer sent by TradingView's `translate` endpoint.
    #[serde(default, rename(deserialize = "IL"))]
    pub il: Ustr,
    /// No longer sent at the top level by TradingView's `translate` endpoint;
    /// use [`Self::script_text`], which also checks its current location.
    #[serde(default, rename(deserialize = "ilTemplate"))]
    pub il_template: Ustr,
    #[serde(rename(deserialize = "metaInfo"))]
    pub data: PineMetadataInfo,
}

impl PineMetadata {
    /// Encrypted script template for the `text` input of `create_study`.
    ///
    /// TradingView moved it from the top-level `ilTemplate` field to
    /// `metaInfo.defaults.inputs.text`. Without it a study is accepted and
    /// completes, but silently produces no data.
    pub fn script_text(&self) -> &str {
        if !self.il_template.is_empty() {
            return self.il_template.as_str();
        }
        self.data
            .defaults
            .get(&Ustr::from("inputs"))
            .and_then(|inputs| inputs.get("text"))
            .and_then(Value::as_str)
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct PineMetadataInfo {
    pub id: Ustr,
    #[serde(rename(deserialize = "scriptIdPart"))]
    pub script_id: Ustr,
    pub description: Ustr,
    pub short_description: Ustr,
    pub financial_period: Option<FinancialPeriod>,
    pub grouping_key: Ustr,
    pub is_fundamental_study: bool,
    pub is_hidden_study: bool,
    pub is_tv_script: bool,
    pub is_tv_script_stub: bool,
    pub is_price_study: bool,
    pub inputs: Vec<PineInput>,
    pub defaults: HashMap<Ustr, Value>,
    pub palettes: HashMap<Ustr, Value>,
    pub pine: HashMap<Ustr, Ustr>,
    pub plots: Vec<Plot>,
    pub styles: HashMap<Ustr, Value>,
    pub uses_private_lib: bool,
    pub warnings: Vec<Value>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, Default, Copy)]
#[serde(default, rename_all = "camelCase")]
pub struct Plot {
    pub id: Ustr,
    pub plot_type: Ustr,
    pub target: Option<Ustr>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct PineInput {
    pub name: Ustr,
    pub inline: Ustr,
    pub id: Ustr,
    pub defval: Value,
    pub is_hidden: bool,
    pub is_fake: bool,
    pub optional: bool,
    pub options: Vec<Ustr>,
    pub tooltip: Option<Ustr>,
    #[serde(rename(deserialize = "type"))]
    pub input_type: Ustr,
}

#[derive(Debug, Default, Clone, PartialEq, Deserialize, Serialize, Copy)]
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
            //Sessions@tv-basicstudies-187!
            //Dividends@tv-basicstudies-251
            //Splits@tv-basicstudies-251
            //Earnings@tv-basicstudies-251
            //Volume@tv-basicstudies-251 , Opts {"length":20,"col_prev_close":false}
        }
    }
}

impl From<String> for ScriptType {
    fn from(_value: String) -> Self {
        todo!()
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct PineIndicator {
    pub script_id: Ustr,
    pub script_version: Ustr,
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
            script_id: Ustr::from(script_id),
            script_version: Ustr::from(script_version),
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
        let mut inputs: HashMap<Ustr, IndicatorInput> = HashMap::new();
        inputs.insert(
            Ustr::from("text"),
            IndicatorInput::String(Ustr::from(self.metadata.script_text())),
        );
        inputs.insert(
            Ustr::from("pineId"),
            IndicatorInput::String(Ustr::from(&self.script_id)),
        );
        inputs.insert(
            Ustr::from("pineVersion"),
            IndicatorInput::String(Ustr::from(&self.script_version)),
        );
        self.metadata.data.inputs.iter().for_each(|input| {
            if input.id == "text" || input.id == "pineId" || input.id == "pineVersion" {
                return;
            }
            inputs.insert(
                Ustr::from(&input.id),
                IndicatorInput::IndicatorInput(InputValue {
                    v: input.defval.clone(),
                    f: Value::from(input.is_fake),
                    t: Value::from(input.input_type.to_string()),
                }),
            );
        });

        let json_value = serde_json::to_value(inputs)?;
        Ok(json_value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn indicator(metadata: PineMetadata) -> PineIndicator {
        PineIndicator {
            script_id: Ustr::from("STD;Fund_total_revenue_fy"),
            script_version: Ustr::from("81.0"),
            script_type: ScriptType::IntervalScript,
            metadata,
        }
    }

    #[test]
    fn test_script_text_read_from_metainfo_defaults() {
        let response: TranslateResponse = serde_json::from_value(json!({
            "success": true,
            "result": {
                "metaInfo": {
                    "defaults": { "inputs": { "text": "bmI9Ks46_encrypted" } },
                    "inputs": [{ "id": "text", "defval": "", "type": "text" }]
                }
            }
        }))
        .unwrap();

        let inputs = indicator(response.result).to_study_inputs().unwrap();

        assert_eq!(inputs["text"], json!("bmI9Ks46_encrypted"));
    }

    #[test]
    fn test_script_text_prefers_top_level_template() {
        let response: TranslateResponse = serde_json::from_value(json!({
            "success": true,
            "result": {
                "IL": "il",
                "ilTemplate": "top_level",
                "metaInfo": { "defaults": { "inputs": { "text": "nested" } } }
            }
        }))
        .unwrap();

        assert_eq!(response.result.script_text(), "top_level");
    }

    #[test]
    fn test_script_text_empty_when_absent() {
        assert_eq!(PineMetadata::default().script_text(), "");
    }
}
