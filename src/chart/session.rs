use std::collections::HashMap;

use crate::{
    chart::{
        utils::{
            extract_ohlcv_data,
            extract_studies_data,
            get_string_value,
            par_extract_ohlcv_data,
        },
        ChartOptions,
        ChartResponseData,
        ChartSeriesData,
        SeriesCompletedMessage,
        StudyResponseData,
        SymbolInfo,
    },
    models::{ pine_indicator::PineIndicator, Interval, Timezone },
    payload,
    socket::{
        AsyncCallback,
        DataServer,
        Socket,
        SocketMessageDe,
        SocketSession,
        TradingViewDataEvent,
    },
    utils::{ gen_id, gen_session_id, symbol_init },
    error::{ Error, TradingViewError },
    Result,
};
use async_trait::async_trait;
use serde_json::Value;
use tracing::{ debug, error, info, trace, warn };

#[derive(Default)]
pub struct WebSocketsBuilder {
    server: Option<DataServer>,
    auth_token: Option<String>,
    socket: Option<SocketSession>,
}

pub struct WebSocket {
    socket: SocketSession,
    series: Vec<SeriesInfo>,
    series_count: u16,
    studies: HashMap<String, String>,
    studies_count: u16,
    collectors: HashMap<String, ChartSeriesData>,
    callbacks: ChartCallbackFn,
}

#[derive(Debug, Clone, Default)]
struct SeriesInfo {
    id: String,
    chart_session: String,
    options: ChartOptions,
}

pub struct ChartCallbackFn {
    pub on_chart_data: AsyncCallback<ChartSeriesData>,
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

    pub async fn connect(&self, callback: ChartCallbackFn) -> Result<WebSocket> {
        let auth_token = self.auth_token.clone().unwrap_or("unauthorized_user_token".to_string());

        let server = self.server.clone().unwrap_or_default();

        let socket = self.socket
            .clone()
            .unwrap_or(SocketSession::new(server, auth_token.clone()).await?);

        Ok(WebSocket {
            socket,
            collectors: HashMap::new(),
            series: Vec::new(),
            series_count: 0,
            studies_count: 0,
            studies: HashMap::new(),
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
        self.socket.send("set_locale", &payload!("en", "US")).await?;
        Ok(())
    }

    pub async fn set_data_quality(&mut self, data_quality: &str) -> Result<()> {
        self.socket.send("set_data_quality", &payload!(data_quality)).await?;
        Ok(())
    }

    pub async fn set_timezone(&mut self, session: &str, timezone: Timezone) -> Result<()> {
        self.socket.send("switch_timezone", &payload!(session, timezone.to_string())).await?;

        Ok(())
    }

    pub async fn update_auth_token(&mut self, auth_token: &str) -> Result<()> {
        self.socket.update_auth_token(auth_token).await?;
        Ok(())
    }

    pub async fn create_chart_session(&mut self, session: &str) -> Result<()> {
        self.socket.send("chart_create_session", &payload!(session)).await?;
        Ok(())
    }

    pub async fn create_replay_session(&mut self, session: &str) -> Result<()> {
        self.socket.send("replay_create_session", &payload!(session)).await?;
        Ok(())
    }

    pub async fn add_replay_series(
        &mut self,
        session: &str,
        series_id: &str,
        symbol: &str,
        config: &ChartOptions
    ) -> Result<()> {
        self.socket.send(
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
                config.interval.to_string()
            )
        ).await?;
        Ok(())
    }

    pub async fn delete_chart_session_id(&mut self, session: &str) -> Result<()> {
        self.socket.send("chart_delete_session", &payload!(session)).await?;
        Ok(())
    }

    pub async fn delete_replay_session_id(&mut self, session: &str) -> Result<()> {
        self.socket.send("replay_delete_session", &payload!(session)).await?;
        Ok(())
    }

    pub async fn replay_step(&mut self, session: &str, series_id: &str, step: u64) -> Result<()> {
        self.socket.send("replay_step", &payload!(session, series_id, step)).await?;
        Ok(())
    }

    pub async fn replay_start(
        &mut self,
        session: &str,
        series_id: &str,
        interval: Interval
    ) -> Result<()> {
        self.socket.send(
            "replay_start",
            &payload!(session, series_id, interval.to_string())
        ).await?;
        Ok(())
    }

    pub async fn replay_stop(&mut self, session: &str, series_id: &str) -> Result<()> {
        self.socket.send("replay_stop", &payload!(session, series_id)).await?;
        Ok(())
    }

    pub async fn replay_reset(
        &mut self,
        session: &str,
        series_id: &str,
        timestamp: i64
    ) -> Result<()> {
        self.socket.send("replay_reset", &payload!(session, series_id, timestamp)).await?;
        Ok(())
    }

    pub async fn request_more_data(
        &mut self,
        session: &str,
        series_id: &str,
        num: u64
    ) -> Result<()> {
        self.socket.send("request_more_data", &payload!(session, series_id, num)).await?;
        Ok(())
    }

    pub async fn request_more_tickmarks(
        &mut self,
        session: &str,
        series_id: &str,
        num: u64
    ) -> Result<()> {
        self.socket.send("request_more_tickmarks", &payload!(session, series_id, num)).await?;
        Ok(())
    }

    pub async fn create_study(
        &mut self,
        session: &str,
        study_id: &str,
        series_id: &str,
        indicator: PineIndicator
    ) -> Result<()> {
        let inputs = indicator.to_study_inputs()?;
        let payloads: Vec<Value> = vec![
            Value::from(session),
            Value::from(study_id),
            Value::from("st1"),
            Value::from(series_id),
            Value::from(indicator.script_type.to_string()),
            inputs
        ];
        self.socket.send("create_study", &payloads).await?;
        Ok(())
    }

    pub async fn modify_study(
        &mut self,
        session: &str,
        study_id: &str,
        series_id: &str,
        indicator: PineIndicator
    ) -> Result<()> {
        let inputs = indicator.to_study_inputs()?;
        let payloads: Vec<Value> = vec![
            Value::from(session),
            Value::from(study_id),
            Value::from("st1"),
            Value::from(series_id),
            Value::from(indicator.script_type.to_string()),
            inputs
        ];
        self.socket.send("modify_study", &payloads).await?;
        Ok(())
    }

    pub async fn remove_study(&mut self, session: &str, study_id: &str) -> Result<()> {
        self.socket.send("remove_study", &payload!(session, study_id)).await?;
        Ok(())
    }

    pub async fn create_series(
        &mut self,
        session: &str,
        series_id: &str,
        series_version: &str,
        series_symbol_id: &str,
        config: &ChartOptions
    ) -> Result<()> {
        let range = match (&config.range, config.from, config.to) {
            (Some(range), _, _) => range.clone(),
            (None, Some(from), Some(to)) => format!("r,{}:{}", from, to),
            _ => String::default(),
        };
        self.socket.send(
            "create_series",
            &payload!(
                session,
                series_id,
                series_version,
                series_symbol_id,
                config.interval.to_string(),
                config.bar_count,
                range // |r,1626220800:1628640000|1D|5d|1M|3M|6M|YTD|12M|60M|ALL|
            )
        ).await?;
        Ok(())
    }

    pub async fn modify_series(
        &mut self,
        session: &str,
        series_id: &str,
        series_version: &str,
        series_symbol_id: &str,
        config: &ChartOptions
    ) -> Result<()> {
        let range = match (&config.range, config.from, config.to) {
            (Some(range), _, _) => range.clone(),
            (None, Some(from), Some(to)) => format!("r,{}:{}", from, to),
            _ => String::default(),
        };
        self.socket.send(
            "modify_series",
            &payload!(
                session,
                series_id,
                series_version,
                series_symbol_id,
                config.interval.to_string(),
                config.bar_count,
                range // |r,1626220800:1628640000|1D|5d|1M|3M|6M|YTD|12M|60M|ALL|
            )
        ).await?;
        Ok(())
    }

    pub async fn remove_series(&mut self, session: &str, series_id: &str) -> Result<()> {
        self.socket.send("remove_series", &payload!(session, series_id)).await?;
        Ok(())
    }

    pub async fn resolve_symbol(
        &mut self,
        session: &str,
        symbol_series_id: &str,
        symbol: &str,
        config: &ChartOptions,
        replay_session: Option<String>
    ) -> Result<()> {
        self.socket.send(
            "resolve_symbol",
            &payload!(
                session,
                symbol_series_id,
                symbol_init(
                    symbol,
                    config.adjustment.clone(),
                    config.currency,
                    config.session_type.clone(),
                    replay_session
                )?
            )
        ).await?;
        Ok(())
    }

    // End TradingView WebSocket methods

    pub async fn set_market(&mut self, config: ChartOptions) -> Result<()> {
        self.series_count += 1;
        let series_count = self.series_count;
        let symbol_series_id = format!("sds_sym_{}", series_count);
        let series_id = format!("sds_{}", series_count);
        let series_version = format!("s{}", series_count);
        let chart_session = gen_session_id("cs");
        self.create_chart_session(&chart_session).await?;

        if config.replay_mode.unwrap_or_default() && config.replay_from.is_some() {
            let replay_session_id = gen_session_id("rs");
            let replay_series_id = gen_id();
            self.create_replay_session(&replay_session_id).await?;
            self.add_replay_series(
                &replay_session_id,
                &replay_series_id,
                &config.symbol,
                &config
            ).await?;
            self.replay_reset(
                &replay_session_id,
                &replay_series_id,
                config.replay_from.unwrap()
            ).await?;
            self.resolve_symbol(
                &chart_session,
                &symbol_series_id,
                &config.symbol,
                &config,
                Some(replay_session_id.clone())
            ).await?;
        } else {
            self.resolve_symbol(
                &chart_session,
                &symbol_series_id,
                &config.symbol,
                &config,
                None
            ).await?;
        }

        self.create_series(
            &chart_session,
            &series_id,
            &series_version,
            &symbol_series_id,
            &config
        ).await?;

        if let Some(study) = &config.study_config {
            self.studies_count += 1;
            let study_count = self.studies_count;
            let study_id = format!("st{}", study_count);

            let indicator = PineIndicator::build().fetch(
                &study.script_id,
                &study.script_version,
                study.script_type.clone()
            ).await?;

            self.studies.insert(indicator.metadata.data.id.clone(), study_id.clone());

            self.create_study(&chart_session, &study_id, &series_id, indicator).await?;
        }

        let series_info = SeriesInfo {
            chart_session,
            id: series_id,
            options: config,
        };

        self.series.push(series_info);

        Ok(())
    }

    pub async fn delete(&mut self) -> Result<()> {
        for s in self.series.clone() {
            self.delete_chart_session_id(&s.chart_session).await?;
        }
        self.socket.close().await?;
        Ok(())
    }

    pub async fn subscribe(&mut self) {
        self.event_loop(&mut self.socket.clone()).await;
    }

    async fn on_data(&mut self, message: SocketMessageDe) -> Result<()> {
        let mut series_info = self.series.clone();
        for s in series_info.iter_mut() {
            trace!("received v: {:?}, m: {:?}", s, message);
            match message.p[1].get(s.id.as_str()) {
                Some(resp_data) => {
                    let resp = serde_json::from_value::<ChartResponseData>(resp_data.clone())?;
                    let data = if resp.series.len() > 20_000 {
                        par_extract_ohlcv_data(&resp)
                    } else {
                        extract_ohlcv_data(&resp)
                    };
                    debug!("series data extracted: {:?}", data);
                    let k = format!("{}-{}", s.options.symbol, s.options.interval);
                    let collector = self.collectors.entry(k.clone()).or_insert(ChartSeriesData {
                        symbol: s.options.symbol.clone(),
                        interval: s.options.interval,
                        data: Vec::new(),
                    });
                    if
                        s.options.fetch_all_data &&
                        collector.data.len() <= s.options.fetch_data_count
                    {
                        collector.data.extend(data);
                        let replay_from = collector.data.first().unwrap().timestamp / 1000;
                        info!("replay loading data from timestamp in seconds: {}", replay_from);
                        self.set_market(ChartOptions {
                            replay_mode: Some(true),
                            replay_from: Some(replay_from),
                            ..s.options.clone()
                        }).await?;
                    } else if
                        s.options.fetch_all_data &&
                        collector.data.len() >= s.options.fetch_data_count
                    {
                        tokio::spawn((self.callbacks.on_chart_data)(collector.clone()));
                        s.options.fetch_all_data = false;
                        collector.data.clear();
                    } else {
                        tokio::spawn(
                            (self.callbacks.on_chart_data)(ChartSeriesData {
                                symbol: collector.symbol.clone(),
                                interval: collector.interval,
                                data,
                            })
                        );
                    }
                }
                None => {
                    debug!("receive empty data on series: {:?}", s);
                }
            }
        }
        if self.series_count != 0 {
            for (k, v) in &self.studies {
                if let Some(resp_data) = message.p[1].get(v.as_str()) {
                    debug!("study data received: {} - {:?}", k, resp_data);
                    let resp = serde_json::from_value::<StudyResponseData>(resp_data.clone())?;
                    let data = extract_studies_data(&resp);
                    debug!("study data extracted: {} - {:?}", k, data);
                }
            }
        }
        Ok(())
    }
}

#[async_trait]
impl Socket for WebSocket {
    async fn handle_message_data(&mut self, message: SocketMessageDe) -> Result<()> {
        match TradingViewDataEvent::from(message.m.clone()) {
            TradingViewDataEvent::OnChartData | TradingViewDataEvent::OnChartDataUpdate => {
                trace!("received chart data: {:?}", message);
                self.on_data(message).await?;
            }
            TradingViewDataEvent::OnSymbolResolved => {
                let symbol_info = serde_json::from_value::<SymbolInfo>(message.p[2].clone())?;
                debug!("received symbol information: {:?}", symbol_info);
                (self.callbacks.on_symbol_resolved)(symbol_info).await;
            }
            TradingViewDataEvent::OnSeriesCompleted => {
                let message = SeriesCompletedMessage {
                    session: get_string_value(&message.p, 0),
                    id: get_string_value(&message.p, 1),
                    update_mode: get_string_value(&message.p, 2),
                    version: get_string_value(&message.p, 3),
                };
                info!("series is completed: {:#?}", message);
                tokio::spawn((self.callbacks.on_series_completed)(message));
            }
            TradingViewDataEvent::OnSeriesLoading => {
                trace!("series is loading: {:#?}", message);
            }
            TradingViewDataEvent::OnReplayResolutions => {
                trace!("received replay resolutions: {:?}", message);
            }
            TradingViewDataEvent::OnReplayPoint => {
                trace!("received replay point: {:?}", message);
            }
            TradingViewDataEvent::OnReplayOk => {
                trace!("received replay ok: {:?}", message);
            }
            TradingViewDataEvent::OnReplayInstanceId => {
                trace!("received replay instance id: {:?}", message);
            }
            TradingViewDataEvent::OnReplayDataEnd => {
                trace!("received replay data end: {:?}", message);
            }
            TradingViewDataEvent::OnStudyLoading => {
                trace!("received study loading message: {:?}", message);
            }
            TradingViewDataEvent::OnStudyCompleted => {
                trace!("received study completed message: {:?}", message);
            }
            TradingViewDataEvent::OnError(error) => {
                match error {
                    TradingViewError::SeriesError => todo!(),
                    TradingViewError::SymbolError => todo!(),
                    TradingViewError::CriticalError => todo!(),
                    TradingViewError::StudyError => todo!(),
                    TradingViewError::ProtocolError => {
                        error!("received protocol error: {:?}, try to reconnect", message);
                        self.socket.reconnect().await?;
                    }
                    TradingViewError::QuoteDataStatusError => todo!(),
                    TradingViewError::ReplayError =>
                        warn!("received replay error {:?}, retry in the next request...", message),
                }
            }
            TradingViewDataEvent::UnknownEvent(e) => {
                warn!("received unknown event: {:?}", e);
            }
            _ => {
                debug!("unhandled event on this session: {:?}", message);
            }
        }
        Ok(())
    }

    async fn handle_error(&mut self, error: Error) {
        match error {
            Error::TradingViewError(e) => {
                match e {
                    TradingViewError::QuoteDataStatusError => {
                        warn!("received quote data status error, reconnecting...");
                        // self.socket.reconnect().await.unwrap();
                    }
                    _ => {
                        error!("received error: {:?}", e);
                    }
                }
            }
            _ => {
                error!("received error: {:?}", error);
            }
        }
        // TODO: better handle error
    }
}
