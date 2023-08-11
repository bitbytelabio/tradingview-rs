use crate::{
    payload,
    prelude::*,
    socket::{DataServer, Socket, SocketEvent, SocketMessage, SocketMessageDe, SocketMessageSer},
    utils::{gen_session_id, parse_packet, symbol_init},
    Interval, MarketAdjustment, SessionType, Timezone,
};
use async_trait::async_trait;
use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use iso_currency::Currency;
use serde_json::Value;
use std::{rc::Rc, sync::Arc};
use tokio::{net::TcpStream, sync::RwLock};
use tokio_tungstenite::{tungstenite::protocol::Message, MaybeTlsStream, WebSocketStream};
use tracing::{debug, error, info, trace, warn};

#[derive(Default)]
pub struct WebSocketsBuilder {
    server: Option<DataServer>,
    auth_token: Option<String>,
    relay_mode: bool,
}

pub struct WebSocket {
    pub write: Arc<RwLock<SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>>>,
    pub read: Arc<RwLock<SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>>>,
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

        let (write_stream, read_stream) = WebSocket::connect(&server, &auth_token).await?;
        let write = Arc::from(RwLock::new(write_stream));
        let read = Arc::from(RwLock::new(read_stream));

        Ok(WebSocket {
            write,
            read,
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
        self.send("set_locale", &payload!("en", "US")).await?;
        Ok(())
    }

    pub async fn set_data_quality(&mut self, data_quality: &str) -> Result<()> {
        self.send("set_data_quality", &payload!(data_quality))
            .await?;
        Ok(())
    }

    pub async fn set_timezone(&mut self, timezone: Timezone) -> Result<()> {
        self.send(
            "switch_timezone",
            &payload!(self.chart_session_id.clone(), timezone.to_string()),
        )
        .await?;

        Ok(())
    }

    pub async fn update_auth_token(&mut self, auth_token: &str) -> Result<()> {
        self.auth_token = auth_token.to_owned();
        self.send("set_auth_token", &payload!(auth_token)).await?;
        Ok(())
    }

    pub async fn create_chart_session(&mut self) -> Result<()> {
        self.chart_session_id = gen_session_id("cs");
        self.send(
            "chart_create_session",
            &payload!(self.chart_session_id.clone()),
        )
        .await?;
        Ok(())
    }

    pub async fn create_replay_session(&mut self) -> Result<()> {
        self.chart_session_id = gen_session_id("rs");
        self.send(
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
        self.send(
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
        self.send(
            "chart_delete_session",
            &payload!(self.chart_session_id.clone()),
        )
        .await?;
        Ok(())
    }

    pub async fn _delete_replay_session_id(&mut self) -> Result<()> {
        self.send(
            "replay_delete_session",
            &payload!(self.chart_session_id.clone()),
        )
        .await?;
        Ok(())
    }

    pub async fn replay_step(&mut self, step: u64) -> Result<()> {
        self.send(
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
        self.send(
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
        self.send(
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
        self.send(
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
        self.send(
            "request_more_data",
            &payload!(self.chart_session_id.clone(), series_id, num),
        )
        .await?;
        Ok(())
    }

    pub async fn request_more_tickmarks(&mut self, series_id: &str, num: u64) -> Result<()> {
        self.send(
            "request_more_tickmarks",
            &payload!(self.chart_session_id.clone(), series_id, num),
        )
        .await?;
        Ok(())
    }

    pub async fn create_study(&mut self, study_id: &str, series_id: &str) -> Result<()> {
        self.send(
            "create_study",
            &payload!(self.chart_session_id.clone(), study_id, series_id),
            //TODO: add study options
        )
        .await?;
        Ok(())
    }

    pub async fn modify_study(&mut self, study_id: &str, options: &str) -> Result<()> {
        self.send(
            "modify_study",
            &payload!(self.chart_session_id.clone(), study_id, options),
            //TODO: add study options
        )
        .await?;
        Ok(())
    }

    pub async fn remove_study(&mut self, study_id: &str) -> Result<()> {
        self.send(
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
        self.send(
            "create_series",
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

    pub async fn modify_series(
        &mut self,
        series_id: &str,
        series_version: &str,
        series_symbol_id: &str,
        interval: Interval,
        bar_count: u64,
    ) -> Result<()> {
        self.send(
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
        self.send(
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
        self.send(
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
    async fn send(&mut self, m: &str, p: &[Value]) -> Result<()> {
        self.write
            .write()
            .await
            .send(SocketMessageSer::new(m, p).to_message()?)
            .await?;
        Ok(())
    }

    async fn ping(&mut self, ping: &Message) -> Result<()> {
        self.write.write().await.send(ping.clone()).await?;
        Ok(())
    }

    async fn close(&mut self) -> Result<()> {
        self.write.write().await.close().await?;
        Ok(())
    }

    async fn event_loop(&mut self) {
        let read = self.read.clone();
        let mut read_guard = read.write().await;

        loop {
            debug!("waiting for next message");
            match read_guard.next().await {
                Some(Ok(message)) => match &message {
                    Message::Text(text) => {
                        trace!("parsing message: {:?}", text);
                        match parse_packet(text) {
                            Ok(values) => {
                                for value in values {
                                    match value {
                                        SocketMessage::SocketServerInfo(info) => {
                                            info!("received server info: {:?}", info);
                                        }
                                        SocketMessage::SocketMessage(msg) => {
                                            if let Err(e) = self.handle_event(msg).await {
                                                error!("Error handling message: {:#?}", e);
                                            }
                                        }
                                        SocketMessage::Other(value) => {
                                            trace!("receive message: {:?}", value);
                                            if value.is_number() {
                                                trace!("handling ping message: {:?}", message);
                                                if let Err(e) = self.ping(&message).await {
                                                    error!("error handling ping: {:#?}", e);
                                                }
                                            } else {
                                                warn!("unhandled message: {:?}", value)
                                            }
                                        }
                                        SocketMessage::Unknown(s) => {
                                            warn!("unknown message: {:?}", s);
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                error!("Error parsing message: {:#?}", e);
                            }
                        }
                    }
                    _ => {
                        warn!("received non-text message: {:?}", message);
                    }
                },
                Some(Err(e)) => {
                    error!("Error reading message: {:#?}", e);
                    break;
                }
                None => {
                    debug!("No more messages to read");
                    break;
                }
            }
        }
    }

    async fn handle_event(&mut self, message: SocketMessageDe) -> Result<()> {
        let message = Rc::new(message);
        match SocketEvent::from(message.m.clone()) {
            _ => {
                trace!("received message: {:#?}", message);
            }
        };
        Ok(())
    }
}
