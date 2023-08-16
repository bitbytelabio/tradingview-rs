use std::{collections::HashMap, sync::Arc};

use crate::{
    chart::{
        utils::{extract_ohlcv_data, get_string_value},
        ChartDataResponse, ChartSeries, SeriesCompletedMessage, SymbolInfo,
    },
    models::{Interval, MarketAdjustment, SessionType, Timezone},
    payload,
    prelude::*,
    socket::{AsyncCallback, DataServer, Socket, SocketEvent, SocketMessageDe, SocketSession},
    tools::par_extract_ohlcv_data,
    utils::{gen_id, gen_session_id, symbol_init},
};
use async_trait::async_trait;
use iso_currency::Currency;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, trace, warn};

#[derive(Default)]
pub struct WebSocketsBuilder {
    server: Option<DataServer>,
    auth_token: Option<String>,
    socket: Option<SocketSession>,
    relay_mode: bool,
}

pub struct WebSocket {
    socket: SocketSession,
    series_info: HashMap<String, ChartSeriesInfo>,
    series_count: u16,
    replay_mode: bool,
    replay_info: HashMap<String, ReplayInfo>,
    auth_token: String,
    callbacks: ChartCallbackFn,
}

#[derive(Debug, Clone, Default)]
struct ChartSeriesInfo {
    id: String,
    _chart_session: String,
    _version: String,
    _symbol_series_id: String,
    _replay_info: Option<ReplayInfo>,
    chart_series: ChartSeries,
}

#[derive(Debug, Clone, Default)]
struct ReplayInfo {
    _timestamp: i64,
    _replay_session_id: String,
    _replay_series_id: String,
    _resolution: Interval,
}

#[derive(Debug, Clone, Default)]
struct StudyInfo {
    _id: String,
    _indicator: String,
}

#[derive(Default, Clone)]
pub struct Options {
    pub resolution: Interval,
    pub bar_count: u64,
    pub range: Option<String>,
    pub from: Option<u64>,
    pub to: Option<u64>,
    pub replay_mode: Option<bool>,
    pub replay_from: Option<i64>,
    pub adjustment: Option<MarketAdjustment>,
    pub currency: Option<Currency>,
    pub session_type: Option<SessionType>,
}

pub struct ChartCallbackFn {
    pub on_chart_data: AsyncCallback<ChartSeries>,
    pub on_symbol_resolved: AsyncCallback<SymbolInfo>,
    pub on_series_completed: AsyncCallback<SeriesCompletedMessage>,
}

impl WebSocketsBuilder {
    pub fn socket(&mut self, socket: SocketSession) -> &mut Self {
        self.socket = Some(socket);
        self
    }

    pub fn server(&mut self, server: DataServer) -> &mut Self {
        self.server = Some(server);
        self
    }

    pub fn auth_token(&mut self, auth_token: String) -> &mut Self {
        self.auth_token = Some(auth_token);
        self
    }

    pub fn relay_mode(&mut self, relay_mode: bool) -> &mut Self {
        self.relay_mode = relay_mode;
        self
    }

    pub async fn connect(&self, callback: ChartCallbackFn) -> Result<WebSocket> {
        let auth_token = self
            .auth_token
            .clone()
            .unwrap_or("unauthorized_user_token".to_string());

        let server = self.server.clone().unwrap_or_default();

        let socket = self
            .socket
            .clone()
            .unwrap_or(SocketSession::new(server, auth_token.clone()).await?);

        Ok(WebSocket {
            socket,
            replay_mode: self.relay_mode,
            series_info: HashMap::new(),
            replay_info: HashMap::new(),
            series_count: 0,
            auth_token,
            callbacks: callback,
        })
    }
}

impl WebSocket {
    pub fn build() -> WebSocketsBuilder {
        WebSocketsBuilder::default()
    }

    // Begin TradingView WebSocket methods

    pub async fn set_locale(&mut self) -> Result<()> {
        self.socket
            .send("set_locale", &payload!("en", "US"))
            .await?;
        Ok(())
    }

    pub async fn set_data_quality(&mut self, data_quality: &str) -> Result<()> {
        self.socket
            .send("set_data_quality", &payload!(data_quality))
            .await?;
        Ok(())
    }

    pub async fn set_timezone(&mut self, session: &str, timezone: Timezone) -> Result<()> {
        self.socket
            .send("switch_timezone", &payload!(session, timezone.to_string()))
            .await?;

        Ok(())
    }

    pub async fn update_auth_token(&mut self, auth_token: &str) -> Result<()> {
        self.auth_token = auth_token.to_owned();
        self.socket
            .send("set_auth_token", &payload!(auth_token))
            .await?;
        Ok(())
    }

    pub async fn create_chart_session(&mut self, session: &str) -> Result<()> {
        self.socket
            .send("chart_create_session", &payload!(session))
            .await?;
        Ok(())
    }

    pub async fn create_replay_session(&mut self, session: &str) -> Result<()> {
        self.socket
            .send("replay_create_session", &payload!(session))
            .await?;
        Ok(())
    }

    pub async fn add_replay_series(
        &mut self,
        session: &str,
        series_id: &str,
        symbol: &str,
        config: &Options,
    ) -> Result<()> {
        self.socket
            .send(
                "replay_add_series",
                &payload!(
                    session,
                    series_id,
                    symbol_init(
                        symbol,
                        config.adjustment.clone(),
                        config.currency,
                        config.session_type.clone(),
                        None
                    )?,
                    config.resolution.to_string()
                ),
            )
            .await?;
        Ok(())
    }

    pub async fn delete_chart_session_id(&mut self, session: &str) -> Result<()> {
        self.socket
            .send("chart_delete_session", &payload!(session))
            .await?;
        Ok(())
    }

    pub async fn delete_replay_session_id(&mut self, session: &str) -> Result<()> {
        self.socket
            .send("replay_delete_session", &payload!(session))
            .await?;
        Ok(())
    }

    pub async fn replay_step(&mut self, session: &str, series_id: &str, step: u64) -> Result<()> {
        self.socket
            .send("replay_step", &payload!(session, series_id, step))
            .await?;
        Ok(())
    }

    pub async fn replay_start(
        &mut self,
        session: &str,
        series_id: &str,
        interval: Interval,
    ) -> Result<()> {
        self.socket
            .send(
                "replay_start",
                &payload!(session, series_id, interval.to_string()),
            )
            .await?;
        Ok(())
    }

    pub async fn replay_stop(&mut self, session: &str, series_id: &str) -> Result<()> {
        self.socket
            .send("replay_stop", &payload!(session, series_id))
            .await?;
        Ok(())
    }

    pub async fn replay_reset(
        &mut self,
        session: &str,
        series_id: &str,
        timestamp: i64,
    ) -> Result<()> {
        self.socket
            .send("replay_reset", &payload!(session, series_id, timestamp))
            .await?;
        Ok(())
    }

    pub async fn request_more_data(
        &mut self,
        session: &str,
        series_id: &str,
        num: u64,
    ) -> Result<()> {
        self.socket
            .send("request_more_data", &payload!(session, series_id, num))
            .await?;
        Ok(())
    }

    pub async fn request_more_tickmarks(
        &mut self,
        session: &str,
        series_id: &str,
        num: u64,
    ) -> Result<()> {
        self.socket
            .send("request_more_tickmarks", &payload!(session, series_id, num))
            .await?;
        Ok(())
    }

    pub async fn create_study(
        &mut self,
        session: &str,
        study_id: &str,
        series_id: &str,
    ) -> Result<()> {
        self.socket
            .send(
                "create_study",
                &payload!(session, study_id, "st1", series_id),
                //TODO: add study options
            )
            .await?;
        Ok(())
    }

    pub async fn modify_study(
        &mut self,
        session: &str,
        study_id: &str,
        options: &str,
    ) -> Result<()> {
        self.socket
            .send(
                "modify_study",
                &payload!(session, study_id, "st1", options),
                //TODO: add study options
            )
            .await?;
        Ok(())
    }

    pub async fn remove_study(&mut self, session: &str, study_id: &str) -> Result<()> {
        self.socket
            .send("remove_study", &payload!(session, study_id))
            .await?;
        Ok(())
    }

    pub async fn create_series(
        &mut self,
        session: &str,
        series_id: &str,
        series_version: &str,
        series_symbol_id: &str,
        config: &Options,
    ) -> Result<()> {
        let range = match (&config.range, config.from, config.to) {
            (Some(range), _, _) => range.clone(),
            (None, Some(from), Some(to)) => format!("r,{}:{}", from, to),
            _ => String::default(),
        };
        self.socket
            .send(
                "create_series",
                &payload!(
                    session,
                    series_id,
                    series_version,
                    series_symbol_id,
                    config.resolution.to_string(),
                    config.bar_count,
                    range // |r,1626220800:1628640000|1D|5d|1M|3M|6M|YTD|12M|60M|ALL|
                ),
            )
            .await?;
        Ok(())
    }

    pub async fn modify_series(
        &mut self,
        session: &str,
        series_id: &str,
        series_version: &str,
        series_symbol_id: &str,
        config: &Options,
    ) -> Result<()> {
        let range = match (&config.range, config.from, config.to) {
            (Some(range), _, _) => range.clone(),
            (None, Some(from), Some(to)) => format!("r,{}:{}", from, to),
            _ => String::default(),
        };
        self.socket
            .send(
                "modify_series",
                &payload!(
                    session,
                    series_id,
                    series_version,
                    series_symbol_id,
                    config.resolution.to_string(),
                    config.bar_count,
                    range // |r,1626220800:1628640000|1D|5d|1M|3M|6M|YTD|12M|60M|ALL|
                ),
            )
            .await?;
        Ok(())
    }

    pub async fn remove_series(&mut self, session: &str, series_id: &str) -> Result<()> {
        self.socket
            .send("remove_series", &payload!(session, series_id))
            .await?;
        Ok(())
    }

    pub async fn resolve_symbol(
        &mut self,
        session: &str,
        symbol_series_id: &str,
        symbol: &str,
        config: &Options,
        replay_session: Option<String>,
    ) -> Result<()> {
        self.socket
            .send(
                "resolve_symbol",
                &payload!(
                    session,
                    symbol_series_id,
                    symbol_init(
                        symbol,
                        config.adjustment.clone(),
                        config.currency,
                        config.session_type.clone(),
                        replay_session,
                    )?
                ),
            )
            .await?;
        Ok(())
    }

    // End TradingView WebSocket methods

    pub async fn set_market(&mut self, symbol: &str, config: Options) -> Result<()> {
        self.series_count += 1;
        let series_count = self.series_count;
        let symbol_series_id = format!("sds_sym_{}", series_count);
        let series_id = format!("sds_{}", series_count);
        let series_version = format!("s{}", series_count);
        let chart_session = gen_session_id("cs");

        self.replay_mode = config.replay_mode.unwrap_or_default();

        self.create_chart_session(&chart_session).await?;

        if self.replay_mode && config.replay_from.is_some() {
            let replay_session_id = gen_session_id("rs");
            let replay_series_id = gen_id();

            self.create_replay_session(&replay_session_id).await?;
            self.add_replay_series(&replay_session_id, &replay_series_id, symbol, &config)
                .await?;
            self.replay_reset(
                &replay_session_id,
                &replay_series_id,
                config.replay_from.unwrap(),
            )
            .await?;

            self.resolve_symbol(
                &chart_session,
                &symbol_series_id,
                symbol,
                &config,
                Some(replay_session_id.clone()),
            )
            .await?;

            self.replay_info.insert(
                symbol.to_string(),
                ReplayInfo {
                    _replay_session_id: replay_session_id,
                    _replay_series_id: replay_series_id,
                    _timestamp: config.replay_from.unwrap(),
                    ..Default::default()
                },
            );
        } else {
            self.resolve_symbol(&chart_session, &symbol_series_id, symbol, &config, None)
                .await?;
        }

        self.create_series(
            &chart_session,
            &series_id,
            &series_version,
            &symbol_series_id,
            &config,
        )
        .await?;

        let series_info = ChartSeriesInfo {
            _chart_session: chart_session,
            id: series_id,
            _version: series_version,
            _symbol_series_id: symbol_series_id,
            chart_series: ChartSeries {
                symbol_id: symbol.to_string(),
                interval: config.resolution,
                ..Default::default()
            },
            ..Default::default()
        };

        self.series_info.insert(symbol.to_string(), series_info);

        Ok(())
    }

    pub async fn subscribe(&mut self) {
        self.event_loop(&mut self.socket.clone()).await;
    }
}

#[async_trait]
impl Socket for WebSocket {
    async fn handle_message_data(&mut self, message: SocketMessageDe) -> Result<()> {
        match SocketEvent::from(message.m.clone()) {
            SocketEvent::OnChartData | SocketEvent::OnChartDataUpdate => {
                trace!("received chart data: {:?}", message);
                let mut tasks: Vec<JoinHandle<Result<Option<ChartSeries>>>> = Vec::new();
                let message = Arc::new(message);
                for (key, value) in &self.series_info {
                    trace!("received k: {}, v: {:?}, m: {:?}", key, value, message);
                    let key = Arc::new(key.clone());
                    let value = Arc::new(value.clone());
                    let message = Arc::clone(&message);
                    let task = tokio::task::spawn(async move {
                        if let Some(data) = message.p[1].get(value.id.as_str()) {
                            let csd = serde_json::from_value::<ChartDataResponse>(data.clone())?;
                            let resp_data = if csd.series.len() > 20_000 {
                                par_extract_ohlcv_data(&csd)
                            } else {
                                extract_ohlcv_data(&csd)
                            };
                            debug!("series data received: {} - {:?}", key, data);
                            Ok(Some(ChartSeries {
                                symbol_id: key.to_string(),
                                interval: value.chart_series.interval,
                                data: resp_data,
                            }))
                        } else {
                            Ok(None)
                        }
                    });
                    tasks.push(task);
                }
                for handler in tasks {
                    if let Some(data) = handler.await?? {
                        (self.callbacks.on_chart_data)(data).await?;
                    }
                }
            }
            SocketEvent::OnSymbolResolved => {
                let symbol_info = serde_json::from_value::<SymbolInfo>(message.p[2].clone())?;
                debug!("received symbol information: {:?}", symbol_info);
                (self.callbacks.on_symbol_resolved)(symbol_info).await?;
            }
            SocketEvent::OnSeriesCompleted => {
                let message = SeriesCompletedMessage {
                    session: get_string_value(&message.p, 0),
                    id: get_string_value(&message.p, 1),
                    update_mode: get_string_value(&message.p, 2),
                    version: get_string_value(&message.p, 3),
                };
                info!("series is completed: {:#?}", message);
                (self.callbacks.on_series_completed)(message).await?;
            }
            SocketEvent::OnSeriesLoading => {
                trace!("series is loading: {:#?}", message);
            }
            SocketEvent::OnReplayResolutions => {
                info!("received replay resolutions: {:?}", message);
            }
            SocketEvent::OnReplayPoint => {
                info!("received replay point: {:?}", message);
            }
            SocketEvent::OnReplayOk => {
                info!("received replay ok: {:?}", message);
            }
            SocketEvent::OnReplayInstanceId => {
                info!("received replay instance id: {:?}", message);
            }
            SocketEvent::OnReplayDataEnd => {
                info!("received replay data end: {:?}", message);
            }
            SocketEvent::OnStudyLoading => {
                info!("received study loading message: {:?}", message);
            }
            SocketEvent::OnStudyCompleted => {
                info!("received study completed message: {:?}", message);
            }
            SocketEvent::OnError(error) => {
                error!("received error: {:?}", error);
                self.handle_error(Error::TradingViewError(error)).await;
            }
            SocketEvent::UnknownEvent(e) => {
                warn!("received unknown event: {:?}", e);
            }
            _ => {
                debug!("unhandled event on this session: {:?}", message);
            }
        };
        Ok(())
    }

    async fn handle_error(&mut self, error: Error) {
        error!("error handling event: {:#?}", error);
        // TODO: handle error
    }
}
