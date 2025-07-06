use crate::{
    DataPoint, Error, QuoteValue, StudyOptions, StudyResponseData, SymbolInfo,
    websocket::SeriesInfo,
};
use serde_json::Value;

#[derive(Debug, Clone)]
pub enum TradingViewResponse {
    ChartData(SeriesInfo, Vec<DataPoint>),
    QuoteData(QuoteValue),
    StudyData(StudyOptions, StudyResponseData),
    Error(Error, Vec<Value>),
    SymbolInfo(SymbolInfo),
    SeriesCompleted(Vec<Value>),
    SeriesLoading(Vec<Value>),
    QuoteCompleted(Vec<Value>),
    ReplayOk(Vec<Value>),
    ReplayPoint(Vec<Value>),
    ReplayInstanceId(Vec<Value>),
    ReplayResolutions(Vec<Value>),
    ReplayDataEnd(Vec<Value>),
    StudyLoading(Vec<Value>),
    StudyCompleted(Vec<Value>),
    UnknownEvent(String, Vec<Value>),
}
