use serde::{Deserialize, Serialize};

use crate::error::TradingViewError;

pub mod session;

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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChartData {
    pub node: String,
    #[serde(rename(deserialize = "s"))]
    pub series: Vec<SeriesDataPoint>,
    pub t: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SeriesDataPoint {
    #[serde(rename(deserialize = "i"))]
    pub index: u64,
    #[serde(rename(deserialize = "v"))]
    pub value: Vec<f64>,
}
