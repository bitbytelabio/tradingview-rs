use crate::{
    ChartOptions, DataPoint, Error, Interval, QuoteValue, StudyOptions, StudyResponseData,
    SymbolInfo, Timezone, pine_indicator::PineIndicator, websocket::SeriesInfo,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ustr::Ustr;

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    UnknownEvent(Ustr, Vec<Value>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TradingViewCommand {
    Cleanup,
    SetAuthToken {
        auth_token: Ustr,
    },
    SetLocals {
        locals: (Ustr, Ustr),
    },
    SetDataQuality {
        quality: Ustr,
    },
    SetTimeZone {
        session: Ustr,
        time_zone: Timezone,
    },
    CreateQuoteSession,
    DeleteQuoteSession,
    SetQuoteFields,
    QuoteFastSymbols {
        symbols: Vec<Ustr>,
    },
    QuoteRemoveSymbols {
        symbols: Vec<Ustr>,
    },

    CreateChartSession {
        session: Ustr,
    },
    DeleteChartSession {
        session: Ustr,
    },
    RequestMoreData {
        session: Ustr,
        series_id: Ustr,
        bar_count: u64,
    },
    RequestMoreTickMarks {
        session: Ustr,
        series_id: Ustr,
        bar_count: u64,
    },

    CreateStudy {
        session: Ustr,
        study_id: Ustr,
        series_id: Ustr,
        indicator: PineIndicator,
    },
    ModifyStudy {
        session: Ustr,
        study_id: Ustr,
        series_id: Ustr,
        indicator: PineIndicator,
    },
    RemoveStudy {
        session: Ustr,
        study_id: Ustr,
        series_id: Ustr,
    },
    SetStudy {
        study_options: StudyOptions,
        session: Ustr,
        series_id: Ustr,
    },
    CreateSeries {
        session: Ustr,
        series_id: Ustr,
        series_version: Ustr,
        series_symbol_id: Ustr,
        config: ChartOptions,
    },
    ModifySeries {
        session: Ustr,
        series_id: Ustr,
        series_version: Ustr,
        series_symbol_id: Ustr,
        config: ChartOptions,
    },
    RemoveSeries {
        session: Ustr,
        series_id: Ustr,
    },
    CreateReplaySession {
        session: Ustr,
    },
    DeleteReplaySession {
        session: Ustr,
    },
    ResolveSymbol {
        session: Ustr,
        symbol: Ustr,
        exchange: Ustr,
        interval: Interval,
    },
    SetReplayStep {
        session: Ustr,
        series_id: Ustr,
        step: u64,
    },
    StartReplay {
        session: Ustr,
        series_id: Ustr,
        interval: Interval,
    },
    StopReplay {
        session: Ustr,
        series_id: Ustr,
    },
    ResetReplay {
        session: Ustr,
        series_id: Ustr,
        timestamp: i64,
    },
    SetReplay {
        symbol: Ustr,
        options: ChartOptions,
        chart_session: Ustr,
        symbol_series_id: Ustr,
    },
    SetMarket {
        options: ChartOptions,
    },
}
