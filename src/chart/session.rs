use crate::{
    error::TradingViewError,
    payload,
    prelude::*,
    quote::{QuotePayload, QuotePayloadType, QuoteSocketMessage, ALL_QUOTE_FIELDS},
    socket::{DataServer, Socket, SocketMessage, SocketMessageType},
    utils::{gen_session_id, parse_packet},
};
use async_trait::async_trait;
use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use serde_json::Value;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::{
    sync::RwLock,
    time::{timeout, Duration},
};
use tokio_tungstenite::{tungstenite::protocol::Message, MaybeTlsStream, WebSocketStream};
use tracing::{debug, error, info, trace, warn};

#[derive(Default)]
pub struct WebSocketsBuilder {
    server: Option<DataServer>,
    auth_token: Option<String>,
}

pub struct WebSocket {
    write: Arc<RwLock<SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>>>,
    read: Arc<RwLock<SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>>>,
    chart_session_id: String,
    auth_token: String,
    callbacks: ChartCallbackFn,
}

pub struct ChartCallbackFn {
    // pub data: Box<dyn FnMut(QuotePayload) -> Result<()> + Send + Sync>,
    // pub loaded: Box<dyn FnMut(QuoteSocketMessage) -> Result<()> + Send + Sync>,
    // pub error: Box<dyn FnMut(TradingViewError) -> Result<()> + Send + Sync>,
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

    pub async fn connect(&self, callback: ChartCallbackFn) -> Result<WebSocket> {
        let auth_token = self
            .auth_token
            .clone()
            .unwrap_or("unauthorized_user_token".to_string());

        let server = self.server.clone().unwrap_or_default();

        let session = gen_session_id("qs");

        let (write_stream, read_stream) = WebSocket::connect(&server, &auth_token).await?;
        let write = Arc::from(RwLock::new(write_stream));
        let read = Arc::from(RwLock::new(read_stream));

        Ok(WebSocket {
            write,
            read,
            chart_session_id: session,
            auth_token,
            callbacks: callback,
        })
    }
}

impl WebSocket {}

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

    async fn event_loop(&mut self) {}

    async fn handle_message(&mut self, message: Value) -> Result<()> {
        Ok(())
    }
}
