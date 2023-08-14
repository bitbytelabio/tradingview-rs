use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{error::TradingViewError, models::Interval};

pub mod session;
pub(crate) mod utils;

#[derive(Debug)]
pub enum ChartEvent {
    Data,
    DataUpdate,
    SeriesLoading,
    SeriesCompleted,
    Error(TradingViewError),
}

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

#[derive(Debug, Clone, Deserialize)]
pub struct ChartDataResponse {
    #[serde(default)]
    pub node: Option<String>,
    #[serde(rename(deserialize = "s"))]
    pub series: Vec<SeriesDataPoint>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SeriesDataPoint {
    #[serde(rename(deserialize = "i"))]
    pub index: i64,
    #[serde(rename(deserialize = "v"))]
    pub value: (f64, f64, f64, f64, f64, f64),
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChartDataChanges {
    pub changes: Vec<f64>,
    pub index: i64,
    pub index_diff: Vec<Value>,
    pub marks: Vec<Value>,
    pub zoffset: i64,
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct ChartSeries {
    pub symbol_id: String,
    pub interval: Interval,
    pub data: Vec<(f64, f64, f64, f64, f64, f64)>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SymbolInfo {
    #[serde(rename(deserialize = "pro_name"))]
    pub id: String,
    pub name: String,
    pub description: String,
    pub exchange: String,
    #[serde(rename = "listed_exchange")]
    pub listed_exchange: String,
    #[serde(rename = "provider_id")]
    pub provider_id: String,
    #[serde(rename = "base_currency")]
    pub base_currency: String,
    #[serde(rename = "base_currency_id")]
    pub base_currency_id: String,
    #[serde(rename = "currency_id")]
    pub currency_id: String,
    #[serde(rename = "currency_code")]
    pub currency_code: String,
    pub session_holidays: String,
    pub subsessions: Vec<Subsession>,
    pub timezone: String,
    #[serde(rename(deserialize = "type"))]
    pub market_type: String,
    pub typespecs: Vec<String>,
    pub aliases: Vec<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Subsession {
    pub description: String,
    pub id: String,
    pub private: bool,
    pub session: String,
    #[serde(rename(deserialize = "session-display"))]
    pub session_display: String,
}
