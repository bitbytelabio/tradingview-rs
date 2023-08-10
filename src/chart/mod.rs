use crate::error::TradingViewError;

pub mod graphic_parser;
pub mod session;
pub mod study;
pub mod websocket;

const ON_DATA: &str = "timescale_update";
const ON_DATA_UPDATE: &str = "du";
const ON_SERIES_LOADING: &str = "series_loading";
const ON_SERIES_COMPLETED: &str = "series_completed";
const ON_SYMBOL_RESOLVED: &str = "symbol_resolved";

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

