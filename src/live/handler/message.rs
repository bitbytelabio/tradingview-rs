use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use ustr::Ustr;

use crate::{
    ChartOptions, DataPoint, Error, Interval, QuoteValue, Result, StudyOptions, StudyResponseData,
    SymbolInfo, Timezone,
    live::handler::types::CommandRx,
    pine_indicator::PineIndicator,
    websocket::{SeriesInfo, WebSocketClient},
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
        timezone: Timezone,
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

pub struct CommandHandler {
    receiver: CommandRx,
    websocket: Arc<WebSocketClient>,
}

impl CommandHandler {
    pub fn new(receiver: CommandRx, websocket: Arc<WebSocketClient>) -> Self {
        Self {
            receiver,
            websocket,
        }
    }

    pub async fn handle_commands(&mut self) -> Result<()> {
        while let Some(command) = self.receiver.recv().await {
            tracing::info!("Received command: {:?}", command);
            match command {
                Command::Delete => {
                    self.websocket.delete().await?;
                }
                Command::SetAuthToken { auth_token } => {
                    self.websocket.set_auth_token(&auth_token).await?;
                }
                Command::SetLocals { locals } => {
                    self.websocket.set_locale((&locals.0, &locals.1)).await?;
                }
                Command::SetDataQuality { quality } => {
                    self.websocket.set_data_quality(&quality).await?;
                }
                Command::SetTimeZone { session, timezone } => {
                    self.websocket.set_timezone(&session, timezone).await?;
                }
                Command::CreateQuoteSession => {
                    self.websocket.create_quote_session().await?;
                }
                Command::DeleteQuoteSession => {
                    self.websocket.delete_quote_session().await?;
                }
                Command::SetQuoteFields => {
                    self.websocket.set_fields().await?;
                }
                Command::QuoteFastSymbols { symbols } => {
                    let symbols: Vec<_> = symbols.into_iter().map(|s| s.as_str()).collect();
                    self.websocket.fast_symbols(&symbols).await?;
                }
                Command::QuoteRemoveSymbols { symbols } => {
                    let symbols: Vec<_> = symbols.into_iter().map(|s| s.as_str()).collect();
                    self.websocket.remove_symbols(&symbols).await?;
                }
                Command::CreateChartSession { session } => {
                    self.websocket.create_chart_session(&session).await?;
                }
                Command::DeleteChartSession { session } => {
                    self.websocket.delete_chart_session(&session).await?;
                }
                Command::RequestMoreData {
                    session,
                    series_id,
                    bar_count,
                } => {
                    self.websocket
                        .request_more_data(&session, &series_id, bar_count)
                        .await?;
                }
                Command::RequestMoreTickMarks {
                    session,
                    series_id,
                    bar_count,
                } => {
                    self.websocket
                        .request_more_tickmarks(&session, &series_id, bar_count)
                        .await?;
                }
                Command::CreateStudy {
                    session,
                    study_id,
                    series_id,
                    indicator,
                } => {
                    self.websocket
                        .create_study(&session, &study_id, &series_id, indicator)
                        .await?;
                }
                Command::ModifyStudy {
                    session,
                    study_id,
                    series_id,
                    indicator,
                } => {
                    self.websocket
                        .modify_study(&session, &study_id, &series_id, indicator)
                        .await?;
                }
                Command::RemoveStudy {
                    session,
                    study_id,
                    series_id,
                } => {
                    let id = format!("{series_id}_{study_id}");
                    self.websocket.remove_study(&session, &id).await?;
                }
                Command::SetStudy {
                    study_options,
                    session,
                    series_id,
                } => {
                    self.websocket
                        .set_study(study_options, &session, &series_id)
                        .await?;
                }
                Command::CreateSeries {
                    session,
                    series_id,
                    series_version,
                    series_symbol_id,
                    config,
                } => {
                    self.websocket
                        .create_series(
                            &session,
                            &series_id,
                            &series_version,
                            &series_symbol_id,
                            config,
                        )
                        .await?;
                }
                Command::ModifySeries {
                    session,
                    series_id,
                    series_version,
                    series_symbol_id,
                    config,
                } => {
                    self.websocket
                        .modify_series(
                            &session,
                            &series_id,
                            &series_version,
                            &series_symbol_id,
                            config,
                        )
                        .await?;
                }
                Command::RemoveSeries { session, series_id } => {
                    self.websocket.remove_series(&session, &series_id).await?;
                }
                Command::CreateReplaySession { session } => {
                    self.websocket.create_replay_session(&session).await?;
                }
                Command::DeleteReplaySession { session } => {
                    self.websocket.delete_replay_session(&session).await?;
                }
                Command::ResolveSymbol {
                    session,
                    symbol,
                    exchange,
                    opts,
                    replay_session,
                } => {
                    self.websocket
                        .resolve_symbol(
                            &session,
                            &symbol,
                            &exchange,
                            opts,
                            replay_session.as_deref(),
                        )
                        .await?;
                }
                Command::SetReplayStep {
                    session,
                    series_id,
                    step,
                } => {
                    self.websocket
                        .replay_step(&session, &series_id, step)
                        .await?;
                }
                Command::StartReplay {
                    session,
                    series_id,
                    interval,
                } => {
                    self.websocket
                        .replay_start(&session, &series_id, interval)
                        .await?;
                }
                Command::StopReplay { session, series_id } => {
                    self.websocket.replay_stop(&session, &series_id).await?;
                }
                Command::ResetReplay {
                    session,
                    series_id,
                    timestamp,
                } => {
                    self.websocket
                        .replay_reset(&session, &series_id, timestamp)
                        .await?;
                }
                Command::SetReplay {
                    symbol,
                    options,
                    chart_session,
                    symbol_series_id,
                } => {
                    self.websocket
                        .set_replay(&symbol, options, &chart_session, &symbol_series_id)
                        .await?;
                }
                Command::SetMarket { options } => {
                    self.websocket.set_market(options).await?;
                }
            }
        }
        Ok(())
    }
}
