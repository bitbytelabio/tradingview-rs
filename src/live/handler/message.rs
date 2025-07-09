use serde::{Deserialize, Serialize};
use serde_json::Value;
use ustr::{Ustr, ustr};

use crate::chart::options::Range;
use crate::{
    ChartOptions, DataPoint, Error, Interval, QuoteValue, Result, StudyOptions, StudyResponseData,
    SymbolInfo, Timezone, pine_indicator::PineIndicator, websocket::SeriesInfo,
};

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
pub enum Command {
    Delete,
    Ping,
    SetAuthToken {
        auth_token: Ustr,
    },
    SetLocals {
        language: Ustr,
        country: Ustr,
    },
    SetDataQuality {
        quality: Ustr,
    },
    SetTimeZone {
        session: Ustr,
        timezone: Timezone,
    },
    CreateQuoteSession,
    DeleteQuoteSession,
    SetQuoteFields,
    SendRawMessage {
        message: Ustr,
    },
    FastSymbols {
        symbols: Vec<Ustr>,
    },
    AddSymbols {
        symbols: Vec<Ustr>,
    },
    RemoveSymbols {
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
        interval: Interval,
        bar_count: u64,
        range: Option<Range>,
    },
    ModifySeries {
        session: Ustr,
        series_id: Ustr,
        series_version: Ustr,
        series_symbol_id: Ustr,
        interval: Interval,
        bar_count: u64,
        range: Option<Range>,
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
        opts: ChartOptions,
        replay_session: Option<Ustr>,
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

impl Command {
    pub fn ping() -> Self {
        Self::Ping
    }

    pub fn set_auth_token<S: AsRef<str>>(auth_token: S) -> Self {
        Self::SetAuthToken {
            auth_token: ustr(auth_token.as_ref()),
        }
    }

    pub fn set_locals<S: AsRef<str>>(language: S, country: S) -> Self {
        Self::SetLocals {
            language: ustr(language.as_ref()),
            country: ustr(country.as_ref()),
        }
    }

    pub fn set_data_quality<S: AsRef<str>>(quality: S) -> Self {
        Self::SetDataQuality {
            quality: ustr(quality.as_ref()),
        }
    }

    pub fn set_timezone<S: AsRef<str>>(session: S, timezone: Timezone) -> Self {
        Self::SetTimeZone {
            session: ustr(session.as_ref()),
            timezone,
        }
    }

    pub fn create_quote_session() -> Self {
        Self::CreateQuoteSession
    }

    pub fn delete_quote_session() -> Self {
        Self::DeleteQuoteSession
    }

    pub fn set_quote_fields() -> Self {
        Self::SetQuoteFields
    }

    pub fn send_raw_message<S: AsRef<str>>(message: S) -> Self {
        Self::SendRawMessage {
            message: ustr(message.as_ref()),
        }
    }

    /// Create AddSymbols command from string slice
    pub fn add_symbols<S: AsRef<str>>(symbols: &[S]) -> Self {
        Self::AddSymbols {
            symbols: symbols.iter().map(|s| ustr(s.as_ref())).collect(),
        }
    }

    /// Create AddSymbols command from a single symbol
    pub fn add_symbol<S: AsRef<str>>(symbol: S) -> Self {
        Self::AddSymbols {
            symbols: vec![ustr(symbol.as_ref())],
        }
    }

    /// Create RemoveSymbols command from string slice
    pub fn remove_symbols<S: AsRef<str>>(symbols: &[S]) -> Self {
        Self::RemoveSymbols {
            symbols: symbols.iter().map(|s| ustr(s.as_ref())).collect(),
        }
    }

    /// Create RemoveSymbols command from a single symbol
    pub fn remove_symbol<S: AsRef<str>>(symbol: S) -> Self {
        Self::RemoveSymbols {
            symbols: vec![ustr(symbol.as_ref())],
        }
    }

    /// Create SetMarket command with builder pattern
    pub fn set_market(options: ChartOptions) -> Self {
        Self::SetMarket { options }
    }

    /// Create Delete command
    pub fn delete() -> Self {
        Self::Delete
    }
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
    pub fn new(messages: &[Value]) -> Result<Self> {
        const REQUIRED_FIELDS: usize = 3;

        if messages.len() < REQUIRED_FIELDS {
            return Err(Error::Internal(Ustr::from(&format!(
                "Loading message requires {} fields, got {}",
                REQUIRED_FIELDS,
                messages.len()
            ))));
        }

        let session = Ustr::deserialize(&messages[0])?;
        let id1 = Ustr::deserialize(&messages[1])?;
        let id2 = Ustr::deserialize(&messages[2])?;

        let data = LoadingData { session, id1, id2 };

        // Auto-detect type based on ID format
        let msg_type = Self::detect_type(&id1, &id2)?;

        Ok(match msg_type {
            LoadingType::Series => Self::Series(data),
            LoadingType::Study => Self::Study(data),
        })
    }

    fn detect_type(series_id1: &str, series_id2: &str) -> Result<LoadingType> {
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

        Err(Error::Internal(Ustr::from(&format!(
            "Invalid series ID format: {series_id1} and {series_id2}"
        ))))
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
