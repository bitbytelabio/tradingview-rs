use futures::future::BoxFuture;
use std::collections::HashMap;

use crate::{
    chart::{utils::extract_ohlcv_data, ChartDataResponse, ChartSeries},
    models::{Interval, MarketAdjustment, SessionType, Timezone},
    payload,
    prelude::*,
    socket::{DataServer, Socket, SocketEvent, SocketMessageDe, SocketSession},
    utils::{gen_id, gen_session_id, symbol_init},
};
use async_trait::async_trait;
use iso_currency::Currency;
use tracing::{error, info, trace, warn};

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

#[derive(Debug, Default)]
pub struct ChartSeriesInfo {
    id: String,
    chart_session: String,
    version: String,
    symbol_series_id: String,
    replay_info: Option<ReplayInfo>,
    chart_series: ChartSeries,
}

#[derive(Debug, Default)]
pub struct ReplayInfo {
    timestamp: i64,
    replay_session_id: String,
    replay_series_id: String,
    resolution: Interval,
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

type AsyncCallback<T> = Box<
    dyn Fn(T) -> BoxFuture<'static, std::result::Result<(), Box<dyn std::error::Error>>>
        + Send
        + Sync,
>;

pub struct ChartCallbackFn {
    pub on_chart_data: AsyncCallback<ChartSeries>,
    // pub symbol_loaded: Box<dyn FnMut(Value) -> Result<()> + Send + Sync>,
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

    // TradingView WebSocket methods

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
                &payload!(session, study_id, series_id),
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
                &payload!(session, study_id, options),
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
                    range // "r,1626220800:1628640000" || "60M"
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
                    range // "r,1626220800:1628640000" || "60M" || "LASTSESSION"
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

    pub async fn set_market(&mut self, symbol: &str, config: Options) -> Result<()> {
        self.series_count += 1;
        let symbol_series_id = format!("sds_sym_{}", self.series_count);
        let series_id = format!("sds_{}", self.series_count);
        let series_version = format!("s{}", self.series_count);
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
                    replay_session_id,
                    replay_series_id,
                    timestamp: config.replay_from.unwrap(),
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

        self.series_info.insert(
            symbol.to_string(),
            ChartSeriesInfo {
                chart_session,
                id: series_id,
                version: series_version,
                symbol_series_id,
                chart_series: ChartSeries {
                    symbol: symbol.to_string(),
                    interval: config.resolution,
                    currency: config.currency,
                    session_type: config.session_type,
                    ..Default::default()
                },
                ..Default::default()
            },
        );

        Ok(())
    }

    pub async fn subscribe(&mut self) {
        self.event_loop(&mut self.socket.clone()).await;
    }
}

#[async_trait]
impl Socket for WebSocket {
    async fn handle_event(&mut self, message: SocketMessageDe) -> Result<()> {
        match SocketEvent::from(message.m.clone()) {
            SocketEvent::OnChartData => {
                trace!("received chart data: {:?}", message);
                let mut ser_data: ChartSeries = Default::default();
                for (k, v) in &self.series_info {
                    trace!("received k: {}, v: {:?}, m: {:?}", k, v, message);
                    if let Some(data) = message.p[1].get(v.id.as_str()) {
                        match serde_json::from_value::<ChartDataResponse>(data.clone()) {
                            Ok(csd) => {
                                let data = extract_ohlcv_data(&csd);
                                info!("{} - {:?}", k, data);
                                ser_data = ChartSeries {
                                    symbol: k.to_string(),
                                    interval: v.chart_series.interval,
                                    currency: v.chart_series.currency,
                                    session_type: v.chart_series.session_type.clone(),
                                    data,
                                };
                            }
                            Err(e) => {
                                error!("failed to deserialize chart data: {}", e);
                            }
                        }
                    }
                }
                trace!("receive data: {:?}", ser_data);
                // match (self.callbacks.on_chart_data)(ser_data).await {
                //     Ok(_) => {
                //         // TODO: SocketEvent::OnChartDataUpdate => {}
                //     }
                //     Err(e) => {
                //         // TODO: SocketEvent::OnChartDataUpdate => {}
                //         error!("failed to call on_chart_data callback: {}", e);
                //     }
                // };
            }
            SocketEvent::OnSymbolResolved => {
                let json_string = serde_json::to_string(&message.p).unwrap();
                error!("{}", json_string);
            }
            SocketEvent::OnChartDataUpdate => {}
            SocketEvent::OnReplayDataEnd => {}
            _ => {
                warn!("unhandled event: {:?}", message);
            }
        };
        Ok(())
    }

    async fn handle_error(&mut self, error: Error) {
        error!("error handling event: {:#?}", error);
    }
}
