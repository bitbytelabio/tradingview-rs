use crate::{
    payload,
    prelude::*,
    socket::{DataServer, Socket, SocketEvent, SocketMessageDe, SocketSession},
    utils::{gen_session_id, symbol_init},
    Interval, MarketAdjustment, SessionType, Timezone,
};
use async_trait::async_trait;
use iso_currency::Currency;
use std::rc::Rc;
use tracing::{debug, error, info, trace, warn};

#[derive(Default)]
pub struct WebSocketsBuilder {
    server: Option<DataServer>,
    auth_token: Option<String>,
    relay_mode: bool,
}

pub struct WebSocket {
    socket: SocketSession,
    chart_session_id: String,
    replay_session_id: String,
    replay_series_id: String,
    _relay_mode: bool,
    auth_token: String,
    _callbacks: ChartCallbackFn,
}

pub struct ChartCallbackFn {
    // pub series_loaded: Box<dyn FnMut(Value) -> Result<()> + Send + Sync>,
    // pub symbol_loaded: Box<dyn FnMut(Value) -> Result<()> + Send + Sync>,
}

impl WebSocketsBuilder {
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

        let socket = SocketSession::new(auth_token.clone(), server).await?;

        Ok(WebSocket {
            socket,
            chart_session_id: String::default(),
            replay_session_id: String::default(),
            replay_series_id: String::default(),
            _relay_mode: self.relay_mode,
            auth_token,
            _callbacks: callback,
        })
    }
}

impl WebSocket {
    pub fn build() -> WebSocketsBuilder {
        WebSocketsBuilder::default()
    }

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

    pub async fn set_timezone(&mut self, timezone: Timezone) -> Result<()> {
        self.socket
            .send(
                "switch_timezone",
                &payload!(self.chart_session_id.clone(), timezone.to_string()),
            )
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

    pub async fn create_chart_session(&mut self) -> Result<()> {
        self.chart_session_id = gen_session_id("cs");
        self.socket
            .send(
                "chart_create_session",
                &payload!(self.chart_session_id.clone()),
            )
            .await?;
        Ok(())
    }

    pub async fn create_replay_session(&mut self) -> Result<()> {
        self.chart_session_id = gen_session_id("rs");
        self.socket
            .send(
                "replay_create_session",
                &payload!(self.chart_session_id.clone()),
            )
            .await?;
        Ok(())
    }

    pub async fn replay_add_series(
        &mut self,
        symbol: &str,
        interval: Interval,
        adjustment: Option<MarketAdjustment>,
        currency: Option<Currency>,
        session_type: Option<SessionType>,
    ) -> Result<()> {
        self.replay_series_id = gen_session_id("rs");
        self.socket
            .send(
                "replay_add_series",
                &payload!(
                    self.chart_session_id.clone(),
                    self.replay_series_id.clone(),
                    symbol_init(symbol, adjustment, currency, session_type)?,
                    interval.to_string()
                ),
            )
            .await?;
        Ok(())
    }

    pub async fn _delete_chart_session_id(&mut self) -> Result<()> {
        self.socket
            .send(
                "chart_delete_session",
                &payload!(self.chart_session_id.clone()),
            )
            .await?;
        Ok(())
    }

    pub async fn _delete_replay_session_id(&mut self) -> Result<()> {
        self.socket
            .send(
                "replay_delete_session",
                &payload!(self.chart_session_id.clone()),
            )
            .await?;
        Ok(())
    }

    pub async fn replay_step(&mut self, step: u64) -> Result<()> {
        self.socket
            .send(
                "replay_step",
                &payload!(
                    self.replay_session_id.clone(),
                    self.replay_series_id.clone(),
                    step
                ),
            )
            .await?;
        Ok(())
    }

    pub async fn replay_start(&mut self, interval: Interval) -> Result<()> {
        self.socket
            .send(
                "replay_start",
                &payload!(
                    self.replay_session_id.clone(),
                    self.replay_series_id.clone(),
                    interval.to_string()
                ),
            )
            .await?;
        Ok(())
    }

    pub async fn replay_stop(&mut self) -> Result<()> {
        self.socket
            .send(
                "replay_stop",
                &payload!(
                    self.replay_session_id.clone(),
                    self.replay_series_id.clone()
                ),
            )
            .await?;
        Ok(())
    }

    pub async fn replay_reset(&mut self, timestamp: i64) -> Result<()> {
        self.socket
            .send(
                "replay_reset",
                &payload!(
                    self.replay_session_id.clone(),
                    self.replay_series_id.clone(),
                    timestamp
                ),
            )
            .await?;
        Ok(())
    }

    pub async fn request_more_data(&mut self, series_id: &str, num: u64) -> Result<()> {
        self.socket
            .send(
                "request_more_data",
                &payload!(self.chart_session_id.clone(), series_id, num),
            )
            .await?;
        Ok(())
    }

    pub async fn request_more_tickmarks(&mut self, series_id: &str, num: u64) -> Result<()> {
        self.socket
            .send(
                "request_more_tickmarks",
                &payload!(self.chart_session_id.clone(), series_id, num),
            )
            .await?;
        Ok(())
    }

    pub async fn create_study(&mut self, study_id: &str, series_id: &str) -> Result<()> {
        self.socket
            .send(
                "create_study",
                &payload!(self.chart_session_id.clone(), study_id, series_id),
                //TODO: add study options
            )
            .await?;
        Ok(())
    }

    pub async fn modify_study(&mut self, study_id: &str, options: &str) -> Result<()> {
        self.socket
            .send(
                "modify_study",
                &payload!(self.chart_session_id.clone(), study_id, options),
                //TODO: add study options
            )
            .await?;
        Ok(())
    }

    pub async fn remove_study(&mut self, study_id: &str) -> Result<()> {
        self.socket
            .send(
                "remove_study",
                &payload!(self.chart_session_id.clone(), study_id),
            )
            .await?;
        Ok(())
    }

    pub async fn create_series(
        &mut self,
        series_id: &str,
        series_version: &str,
        series_symbol_id: &str,
        interval: Interval,
        bar_count: u64,
    ) -> Result<()> {
        self.socket
            .send(
                "create_series",
                &payload!(
                    self.chart_session_id.clone(),
                    series_id,
                    series_version,
                    series_symbol_id,
                    interval.to_string(),
                    bar_count,
                    "60M" // "r,1626220800:1628640000"
                ),
            )
            .await?;
        Ok(())
    }

    pub async fn modify_series(
        &mut self,
        series_id: &str,
        series_version: &str,
        series_symbol_id: &str,
        interval: Interval,
        bar_count: u64,
    ) -> Result<()> {
        self.socket
            .send(
                "modify_series",
                &payload!(
                    self.chart_session_id.clone(),
                    series_id,
                    series_version,
                    series_symbol_id,
                    interval.to_string(),
                    bar_count
                ),
            )
            .await?;
        Ok(())
    }

    pub async fn remove_series(&mut self, series_id: &str) -> Result<()> {
        self.socket
            .send(
                "remove_series",
                &payload!(self.chart_session_id.clone(), series_id),
            )
            .await?;
        Ok(())
    }

    pub async fn resolve_symbol(
        &mut self,
        symbol_series_id: &str,
        symbol: &str,
        adjustment: Option<MarketAdjustment>,
        currency: Option<Currency>,
        session_type: Option<SessionType>,
    ) -> Result<()> {
        self.socket
            .send(
                "resolve_symbol",
                &payload!(
                    self.chart_session_id.clone(),
                    symbol_series_id,
                    symbol_init(symbol, adjustment, currency, session_type)?
                ),
            )
            .await?;
        Ok(())
    }
}

#[async_trait]
impl Socket for WebSocket {
    async fn handle_event(&mut self, message: SocketMessageDe) -> Result<()> {
        let message = Rc::new(message);
        match SocketEvent::from(message.m.clone()) {
            _ => {
                info!("received message: {:#?}", message);
            }
        };
        Ok(())
    }

    async fn handle_error(&mut self, error: Error) {
        error!("error handling event: {:#?}", error);
    }
}
