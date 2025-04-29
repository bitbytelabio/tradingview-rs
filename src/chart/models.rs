use super::ChartOptions;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[cfg(feature = "technical-analysis")]
use yata::prelude::OHLCV;

pub enum ChartType {
    HeikinAshi,
    Renko,
    LineBreak,
    Kagi,
    PointAndFigure,
    Range,
}

impl std::fmt::Display for ChartType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let chart_type = match self {
            ChartType::HeikinAshi => "BarSetHeikenAshi@tv-basicstudies-60!",
            ChartType::Renko => "BarSetRenko@tv-prostudies-40!",
            ChartType::LineBreak => "BarSetPriceBreak@tv-prostudies-34!",
            ChartType::Kagi => "BarSetKagi@tv-prostudies-34!",
            ChartType::PointAndFigure => "BarSetPnF@tv-prostudies-34!",
            ChartType::Range => "BarSetRange@tv-basicstudies-72!",
        };
        write!(f, "{}", chart_type)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChartHistoricalData {
    pub options: ChartOptions,
    pub data: Vec<DataPoint>,
}

impl ChartHistoricalData {
    pub fn new(opt: &ChartOptions) -> Self {
        Self {
            options: opt.clone(),
            data: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChartResponseData {
    #[serde(default)]
    pub node: Option<String>,
    #[serde(rename(deserialize = "s"))]
    pub series: Vec<DataPoint>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StudyResponseData {
    #[serde(default)]
    pub node: Option<String>,
    #[serde(rename(deserialize = "st"))]
    pub studies: Vec<DataPoint>,
    #[serde(rename(deserialize = "ns"))]
    pub raw_graphics: GraphicDataResponse,
}

// TODO: Implement graphic parser for indexes response
#[derive(Debug, Clone, Deserialize)]
pub struct GraphicDataResponse {
    pub d: String,
    pub indexes: Value,
}

#[cfg_attr(not(feature = "protobuf"), derive(Debug, Default))]
#[cfg_attr(feature = "protobuf", derive(prost::Message))]
#[derive(Clone, Deserialize, Serialize, PartialEq)]
pub struct DataPoint {
    #[cfg_attr(feature = "protobuf", prost(int64, tag = "1"))]
    #[serde(rename(deserialize = "i"))]
    pub index: i64,
    #[cfg_attr(feature = "protobuf", prost(double, repeated, tag = "2"))]
    #[serde(rename(deserialize = "v"))]
    pub value: Vec<f64>,
}

impl DataPoint {}

#[cfg(feature = "technical-analysis")]
impl OHLCV for DataPoint {
    fn open(&self) -> f64 {
        if self.value.len() < 6 {
            return f64::NAN;
        }
        self.value[1]
    }

    fn high(&self) -> f64 {
        if self.value.len() < 6 {
            return f64::NAN;
        }
        self.value[2]
    }

    fn low(&self) -> f64 {
        if self.value.len() < 6 {
            return f64::NAN;
        }
        self.value[3]
    }

    fn close(&self) -> f64 {
        if self.value.len() < 6 {
            return f64::NAN;
        }
        self.value[4]
    }

    fn volume(&self) -> f64 {
        if self.value.len() < 6 {
            return f64::NAN;
        }
        self.value[5]
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChartDataChanges {
    pub changes: Vec<f64>,
    pub index: i64,
    pub index_diff: Vec<Value>,
    pub marks: Vec<Value>,
    pub zoffset: i64,
}

#[cfg_attr(not(feature = "protobuf"), derive(Debug, Default))]
#[cfg_attr(feature = "protobuf", derive(prost::Message))]
#[derive(Clone, PartialEq, Serialize, Hash)]
pub struct SeriesCompletedMessage {
    #[cfg_attr(feature = "protobuf", prost(string, tag = "1"))]
    #[serde(default)]
    pub id: String,
    #[cfg_attr(feature = "protobuf", prost(string, tag = "2"))]
    #[serde(default)]
    pub update_mode: String,
}

#[cfg_attr(not(feature = "protobuf"), derive(Debug, Default))]
#[cfg_attr(feature = "protobuf", derive(prost::Message))]
#[derive(Clone, PartialEq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "camelCase")]
pub struct SymbolInfo {
    #[cfg_attr(feature = "protobuf", prost(string, tag = "1"))]
    #[serde(rename(deserialize = "pro_name"), default)]
    pub id: String,
    #[cfg_attr(feature = "protobuf", prost(string, tag = "2"))]
    #[serde(default)]
    pub name: String,
    #[cfg_attr(feature = "protobuf", prost(string, tag = "3"))]
    #[serde(default)]
    pub description: String,
    #[cfg_attr(feature = "protobuf", prost(string, tag = "4"))]
    pub exchange: String,
    #[cfg_attr(feature = "protobuf", prost(string, tag = "5"))]
    #[serde(rename = "listed_exchange", default)]
    pub listed_exchange: String,
    #[cfg_attr(feature = "protobuf", prost(string, tag = "6"))]
    #[serde(rename = "provider_id", default)]
    pub provider_id: String,
    #[cfg_attr(feature = "protobuf", prost(string, tag = "7"))]
    #[serde(rename = "base_currency", default)]
    pub base_currency: String,
    #[cfg_attr(feature = "protobuf", prost(string, tag = "8"))]
    #[serde(rename = "base_currency_id", default)]
    pub base_currency_id: String,
    #[cfg_attr(feature = "protobuf", prost(string, tag = "9"))]
    #[serde(rename = "currency_id", default)]
    pub currency_id: String,
    #[cfg_attr(feature = "protobuf", prost(string, tag = "10"))]
    #[serde(rename = "currency_code", default)]
    pub currency_code: String,
    #[cfg_attr(feature = "protobuf", prost(string, tag = "11"))]
    #[serde(default)]
    pub session_holidays: String,
    #[cfg_attr(feature = "protobuf", prost(message, repeated, tag = "12"))]
    #[serde(default)]
    pub subsessions: Vec<Subsession>,
    #[cfg_attr(feature = "protobuf", prost(string, tag = "13"))]
    #[serde(default)]
    pub timezone: String,
    #[cfg_attr(feature = "protobuf", prost(string, tag = "14"))]
    #[serde(rename(deserialize = "type"), default)]
    pub market_type: String,
    #[cfg_attr(feature = "protobuf", prost(string, repeated, tag = "15"))]
    #[serde(default)]
    pub typespecs: Vec<String>,
    #[cfg_attr(feature = "protobuf", prost(string, repeated, tag = "16"))]
    #[serde(default)]
    pub aliases: Vec<String>,
}

#[cfg_attr(not(feature = "protobuf"), derive(Debug, Default))]
#[cfg_attr(feature = "protobuf", derive(prost::Message))]
#[derive(Clone, PartialEq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "camelCase")]
pub struct Subsession {
    #[cfg_attr(feature = "protobuf", prost(string, tag = "1"))]
    pub id: String,
    #[cfg_attr(feature = "protobuf", prost(string, tag = "2"))]
    pub description: String,
    #[cfg_attr(feature = "protobuf", prost(bool, tag = "3"))]
    pub private: bool,
    #[cfg_attr(feature = "protobuf", prost(string, tag = "4"))]
    pub session: String,
    #[cfg_attr(feature = "protobuf", prost(string, tag = "5"))]
    #[serde(rename(deserialize = "session-display"))]
    pub session_display: String,
}
