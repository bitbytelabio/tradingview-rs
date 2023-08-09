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
use tokio::time::{timeout, Duration};
use tokio::{net::TcpStream, sync::Mutex};
use tokio_tungstenite::{tungstenite::protocol::Message, MaybeTlsStream, WebSocketStream};
use tracing::{error, info, warn};

#[derive(Default)]
pub struct WebSocketsBuilder {
    server: Option<DataServer>,
    auth_token: Option<String>,
    quote_fields: Option<Vec<String>>,
}

pub struct WebSocket {
    write: Arc<Mutex<SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>>>,
    read: Arc<Mutex<SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>>>,
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

        let write: Arc<Mutex<SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>>> =
            Arc::from(Mutex::new(write_stream));

        let read = Arc::from(Mutex::new(read_stream));

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
    pub fn new() -> WebSocketsBuilder {
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
            .lock()
            .await
            .send(SocketMessage::new(m, p).to_message()?)
            .await?;
        Ok(())
    }

    async fn ping(&mut self, ping: &Message) -> Result<()> {
        self.write.lock().await.send(ping.clone()).await?;
        Ok(())
    }
    async fn event_loop(&mut self) {
        loop {
            let read = self.read.clone();
            let mut read_guard = read.lock().await;
            match timeout(Duration::from_secs(5), read_guard.next()).await {
                Ok(Some(result)) => match result {
                    Ok(message) => {
                        if let Message::Text(text) = &message {
                            if let Ok(values) = parse_packet(text) {
                                for value in values {
                                    match value {
                                        Value::Number(_) => {
                                            if let Err(e) = self.ping(&message).await {
                                                error!("Error handling ping: {:#?}", e);
                                            }
                                        }
                                        Value::Object(_) => {
                                            if let Err(e) = self.handle_message(value).await {
                                                error!("Error handling message: {:#?}", e);
                                            }
                                        }
                                        _ => (),
                                    }
                                }
                            } else if let Err(e) = parse_packet(text) {
                                error!("Error parsing message: {:#?}", e);
                            }
                        } else {
                            warn!("Received non-text message: {:?}", message);
                        }
                    }
                    Err(e) => {
                        error!("Error reading message: {:#?}", e);
                        break;
                    }
                },
                Ok(None) => {
                    // Connection closed
                    break;
                }
                Err(e) => {
                    error!("Error reading message: {:#?}", e);
                    break;
                }
            }
        }
    }

    async fn handle_message(&mut self, message: Value) -> Result<()> {
        let payload: SocketMessageType<QuoteSocketMessage> = match serde_json::from_value(message) {
            Ok(payload) => payload,
            Err(e) => {
                error!("error parsing quote data: {:#?}", e);
                return Err(Error::JsonParseError(e));
            }
        };

        match payload {
            SocketMessageType::SocketServerInfo(server_info) => {
                info!("Received SocketServerInfo: {:#?}", server_info);
            }
            SocketMessageType::SocketMessage(quote_msg) => match quote_msg.message_type.as_str() {
                "qsd" => {
                    if let Some(QuotePayloadType::QuotePayload(quote_payload)) =
                        &quote_msg.payload.get(1)
                    {
                        if quote_payload.status == "ok" {
                            (self.callbacks.data)(*quote_payload.clone())?;
                        } else {
                            (self.callbacks.error)(TradingViewError::QuoteDataStatusError)?;
                        }
                    }
                }
                "quote_completed" => {
                    (self.callbacks.loaded)(quote_msg)?;
                }
                _ => {}
            },
        }

        Ok(())
    }

    async fn handle_error(&mut self, _message: Value) -> Result<()> {
        Ok(())
    }
    async fn handle_data(&mut self, _payload: Value) -> Result<()> {
        Ok(())
    }
    async fn handle_loaded(&mut self, _payload: Value) -> Result<()> {
        Ok(())
    }
}
