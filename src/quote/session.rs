use crate::{
    error::TradingViewError,
    payload,
    prelude::*,
    quote::{QuotePayload, QuotePayloadType, QuoteSocketMessage, ALL_QUOTE_FIELDS},
    socket::{DataServer, Socket, SocketEvent, SocketMessage, SocketMessageDe, SocketMessageSer},
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
use tokio::sync::RwLock;
use tokio_tungstenite::{tungstenite::protocol::Message, MaybeTlsStream, WebSocketStream};
use tracing::{debug, error, info, trace, warn};

#[derive(Default)]
pub struct WebSocketsBuilder {
    server: Option<DataServer>,
    auth_token: Option<String>,
    quote_fields: Option<Vec<String>>,
}

pub struct WebSocket {
    pub write: Arc<RwLock<SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>>>,
    pub read: Arc<RwLock<SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>>>,
    quote_session_id: String,
    quote_fields: Vec<Value>,
    auth_token: String,
    callbacks: QuoteCallbackFn,
}

pub struct QuoteCallbackFn {
    pub data: Box<dyn FnMut(QuotePayload) -> Result<()> + Send + Sync>,
    pub loaded: Box<dyn FnMut(QuoteSocketMessage) -> Result<()> + Send + Sync>,
    pub error: Box<dyn FnMut(TradingViewError) -> Result<()> + Send + Sync>,
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

    pub fn quote_fields(&mut self, quote_fields: Vec<String>) -> &mut Self {
        self.quote_fields = Some(quote_fields);
        self
    }

    pub async fn connect(&self, callback: QuoteCallbackFn) -> Result<WebSocket> {
        let auth_token = self
            .auth_token
            .clone()
            .unwrap_or("unauthorized_user_token".to_string());

        let server = self.server.clone().unwrap_or_default();

        let session = gen_session_id("qs");

        let quote_fields: Vec<Value> = match self.quote_fields.clone() {
            Some(fields) => {
                let mut quote_fields = payload![session.clone().to_string()];
                quote_fields.extend(fields.into_iter().map(Value::from));
                quote_fields
            }
            None => {
                let mut quote_fields = payload![session.clone().to_string()];
                quote_fields.extend(ALL_QUOTE_FIELDS.clone().into_iter().map(Value::from));
                quote_fields
            }
        };

        let (write_stream, read_stream) = WebSocket::connect(&server, &auth_token).await?;
        let write = Arc::from(RwLock::new(write_stream));
        let read = Arc::from(RwLock::new(read_stream));

        Ok(WebSocket {
            write,
            read,
            quote_session_id: session,
            quote_fields,
            auth_token,
            callbacks: callback,
        })
    }
}

impl WebSocket {
    pub fn build() -> WebSocketsBuilder {
        WebSocketsBuilder::default()
    }

    pub async fn create_session(&mut self) -> Result<()> {
        self.send(
            "quote_create_session",
            &payload!(self.quote_session_id.clone()),
        )
        .await?;
        Ok(())
    }

    pub async fn delete_session(&mut self) -> Result<()> {
        self.send(
            "quote_delete_session",
            &payload!(self.quote_session_id.clone()),
        )
        .await?;
        Ok(())
    }

    pub async fn update_session(&mut self) -> Result<()> {
        self.delete_session().await?;
        self.quote_session_id = gen_session_id("qs");
        self.send(
            "quote_create_session",
            &payload!(self.quote_session_id.clone()),
        )
        .await?;
        Ok(())
    }

    pub async fn set_fields(&mut self) -> Result<()> {
        self.send("quote_set_fields", &self.quote_fields.clone())
            .await?;
        Ok(())
    }

    pub async fn add_symbols(&mut self, symbols: Vec<&str>) -> Result<()> {
        let mut payloads = payload![self.quote_session_id.clone()];
        payloads.extend(symbols.into_iter().map(Value::from));
        self.send("quote_add_symbols", &payloads).await?;
        Ok(())
    }

    pub async fn update_auth_token(&mut self, auth_token: &str) -> Result<()> {
        self.auth_token = auth_token.to_owned();
        self.send("set_auth_token", &payload!(auth_token)).await?;
        Ok(())
    }

    pub async fn fast_symbols(&mut self, symbols: Vec<&str>) -> Result<()> {
        let mut payloads = payload![self.quote_session_id.clone()];
        payloads.extend(symbols.into_iter().map(Value::from));
        self.send("quote_fast_symbols", &payloads).await?;
        Ok(())
    }

    pub async fn remove_symbols(&mut self, symbols: Vec<&str>) -> Result<()> {
        let mut payloads = payload![self.quote_session_id.clone()];
        payloads.extend(symbols.into_iter().map(Value::from));
        self.send("quote_remove_symbols", &payloads).await?;
        Ok(())
    }

    pub async fn subscribe(&mut self) {
        self.event_loop().await;
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
                                            if let Err(e) = self.handle_message(msg).await {
                                                error!("Error handling message: {:#?}", e);
                                            }
                                        }
                                        SocketMessage::Other(value) => {
                                            trace!("receive message: {:?}", value);
                                            if value.is_number() {
                                                info!("handling ping message: {:?}", message);
                                                if let Err(e) = self.ping(&message).await {
                                                    error!("Error handling ping: {:#?}", e);
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

    async fn handle_message(&mut self, message: SocketMessageDe) -> Result<()> {
        match SocketEvent::from(message.m.clone()) {
            SocketEvent::OnQuoteData => {
                trace!("received OnQuoteData: {:#?}", message);
                let qsd = serde_json::from_value::<QuotePayload>(message.p[1].clone())?;
                if qsd.status == "ok" {
                    (self.callbacks.data)(qsd)?;
                    return Ok(());
                } else {
                    (self.callbacks.error)(TradingViewError::QuoteDataStatusError)?;
                    return Ok(());
                }
            }
            SocketEvent::OnQuoteCompleted => {
                return Ok(());
            }
            SocketEvent::OnError(e) => {
                (self.callbacks.error)(e)?;
                return Ok(());
            }
            _ => {
                info!("Received SocketMessage");
            }
        };
        Ok(())
    }
}
