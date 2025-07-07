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
    SeriesLoading(LoadingMsg),
    QuoteCompleted(Vec<Value>),
    ReplayOk(Vec<Value>),
    ReplayPoint(Vec<Value>),
    ReplayInstanceId(Vec<Value>),
    ReplayResolutions(Vec<Value>),
    ReplayDataEnd(Vec<Value>),
    StudyLoading(LoadingMsg),
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

#[derive(Debug, Clone, Serialize, Deserialize, Copy, PartialEq, Eq, Hash)]
pub enum LoadingMsg {
    Series(LoadingData),
    Study(LoadingData),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoadingType {
    Series,
    Study,
}

#[derive(Debug, Clone, Serialize, Deserialize, Copy, PartialEq, Eq, Hash)]
pub struct LoadingData {
    pub session: Ustr,
    pub id1: Ustr,
    pub id2: Ustr,
}

impl LoadingMsg {
    pub fn new(messages: &[Value]) -> Result<Self, String> {
        const REQUIRED_FIELDS: usize = 3;

        if messages.len() < REQUIRED_FIELDS {
            return Err(format!(
                "Loading message requires {} fields, got {}",
                REQUIRED_FIELDS,
                messages.len()
            ));
        }

        let session = messages[0]
            .as_str()
            .ok_or("Invalid session field: expected string")?
            .into();

        let id1 = messages[1]
            .as_str()
            .ok_or("Invalid series_id1 field: expected string")?
            .into();

        let id2 = messages[2]
            .as_str()
            .ok_or("Invalid series_id2 field: expected string")?
            .into();

        let data = LoadingData { session, id1, id2 };

        // Auto-detect type based on ID format
        let msg_type = Self::detect_type(&id1, &id2)?;

        Ok(match msg_type {
            LoadingType::Series => Self::Series(data),
            LoadingType::Study => Self::Study(data),
        })
    }

    fn detect_type(series_id1: &str, series_id2: &str) -> Result<LoadingType, String> {
        // Check for series pattern: sds_* and s*
        if series_id1.starts_with("sds_")
            && series_id2.starts_with('s')
            && !series_id2.contains("_st")
        {
            return Ok(LoadingType::Series);
        }

        // Check for study pattern: st* and *_st*
        if series_id1.starts_with("st") && series_id2.contains("_st") {
            return Ok(LoadingType::Study);
        }

        Err(format!(
            "Cannot determine loading type from IDs: '{series_id1}', '{series_id2}'. Expected series pattern (sds_*, s*) or study pattern (st*, *_st*)"
        ))
    }

    pub fn session(&self) -> Ustr {
        match self {
            Self::Series(data) | Self::Study(data) => data.session,
        }
    }

    pub fn id1(&self) -> Ustr {
        match self {
            Self::Series(data) | Self::Study(data) => data.id1,
        }
    }

    pub fn id2(&self) -> Ustr {
        match self {
            Self::Series(data) | Self::Study(data) => data.id2,
        }
    }

    pub fn data(&self) -> &LoadingData {
        match self {
            Self::Series(data) | Self::Study(data) => data,
        }
    }

    pub fn is_series(&self) -> bool {
        matches!(self, Self::Series(_))
    }

    pub fn is_study(&self) -> bool {
        matches!(self, Self::Study(_))
    }
}
