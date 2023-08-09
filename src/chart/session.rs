use crate::{
    error::TradingViewError,
    payload,
    prelude::*,
    socket::{DataServer, Socket, SocketMessage, SocketMessageType},
    utils::{gen_session_id, parse_packet},
    Interval, Timezone,
};
use async_trait::async_trait;
use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use serde_json::Value;
use std::sync::Arc;
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
    write: Arc<RwLock<SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>>>,
    read: Arc<RwLock<SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>>>,
    chart_session_id: String,
    replay_session_id: String,
    replay_series_id: String,
    relay_mode: bool,
    auth_token: String,
    callbacks: ChartCallbackFn,
}

pub struct ChartCallbackFn {
    // pub series_loaded: Box<dyn FnMut(QuotePayload) -> Result<()> + Send + Sync>,
    // pub symbol_loaded: Box<dyn FnMut(QuotePayload) -> Result<()> + Send + Sync>,
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
            relay_mode: self.relay_mode,
            auth_token,
            callbacks: callback,
        })
    }
}

impl WebSocket {
    pub fn new() -> WebSocketsBuilder {
        WebSocketsBuilder::default()
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

    pub async fn set_locale(&mut self) -> Result<()> {
        self.send("set_locale", &payload!("en", "US")).await?;
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

    pub async fn fetch_more_data(&mut self, num: u64) -> Result<()> {
        self.send(
            "request_more_data",
            &payload!(
                self.chart_session_id.clone(),
                "$prices".to_string(),
                num.to_string()
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
            .send(SocketMessage::new(m, p).to_message()?)
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

    async fn event_loop(&mut self) {}

    async fn handle_message(&mut self, message: Value) -> Result<()> {
        Ok(())
    }
}
