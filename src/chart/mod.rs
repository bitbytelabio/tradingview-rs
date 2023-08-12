use serde::Deserialize;
use serde_json::Value;

use crate::error::TradingViewError;

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
pub struct ChartData {
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
