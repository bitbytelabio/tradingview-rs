use crate::{
    error::TradingViewError,
    payload,
    prelude::*,
    quote::{QuoteEvent, ALL_QUOTE_FIELDS},
    socket::{DataServer, SocketMessage},
    utils::{format_packet, gen_session_id, parse_packet},
    WEBSOCKET_HEADERS,
};
use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use serde::Serialize;
use serde_json::Value;
use std::{borrow::Cow, collections::VecDeque};

use tokio::net::TcpStream;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, protocol::Message},
    MaybeTlsStream, WebSocketStream,
};

use tracing::{debug, error, info, warn};
use url::Url;

const MESSAGE_TYPE_KEY: &str = "m";
const PAYLOAD_KEY: usize = 1;
const STATUS_KEY: &str = "s";
const OK_STATUS: &str = "ok";
const ERROR_STATUS: &str = "error";
const DATA_EVENT: &str = "qsd";
const LOADED_EVENT: &str = "quote_completed";
const ERROR_EVENT: &str = "critical_error";

pub struct QuoteSocket<'a> {
    write: SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>,
    read: SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    quote_session_id: String,
    messages: VecDeque<Message>,
    auth_token: String,
    handler: Box<dyn FnMut(QuoteEvent, Value) -> Result<()> + 'a>,
}
pub struct QuoteSocketBuilder<'a> {
    server: DataServer,
    auth_token: Option<String>,
    quote_fields: Option<Vec<String>>,
    handler: Option<Box<dyn FnMut(QuoteEvent, Value) -> Result<()> + 'a>>,
}

impl<'a> QuoteSocketBuilder<'a> {
    pub fn auth_token(&mut self, auth_token: String) -> &mut Self {
        self.auth_token = Some(auth_token);
        self
    }

    pub fn quote_fields(&mut self, quote_fields: Vec<String>) -> &mut Self {
        self.quote_fields = Some(quote_fields);
        self
    }

    fn initial_messages(&self, session: &str, auth_token: &str) -> Result<VecDeque<Message>> {
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

        Ok(VecDeque::from(vec![
            SocketMessage::new("set_auth_token", payload!(auth_token)).to_message()?,
            SocketMessage::new("quote_create_session", payload!(session)).to_message()?,
            SocketMessage::new("quote_set_fields", quote_fields).to_message()?,
        ]))
    }

    pub async fn build(&mut self) -> Result<QuoteSocket> {
        let url = Url::parse(&format!(
            "wss://{server}.tradingview.com/socket.io/websocket",
            server = self.server
        ))
        .unwrap();

        let mut request = url.into_client_request().unwrap();
        request.headers_mut().extend(WEBSOCKET_HEADERS.clone());

        let socket: WebSocketStream<MaybeTlsStream<TcpStream>> = match connect_async(request).await
        {
            Ok(answer) => {
                info!("WebSocket handshake has been successfully completed");
                debug!("WebSocket handshake response: {:?}", answer.1);
                answer.0
            }
            Err(e) => {
                error!("Failed to connect: {}", e);
                return Err(Error::WebSocketError(e));
            }
        };

        let (write, read) = socket.split();

        let auth_token = match self.auth_token.clone() {
            Some(token) => token,
            None => "unauthorized_user_token".to_string(),
        };

        let quote_session_id = gen_session_id("qs");

        let messages = self.initial_messages(&quote_session_id, &auth_token)?;

        Ok(QuoteSocket {
            write,
            read,
            quote_session_id,
            messages,
            auth_token,
            handler: self.handler.take().unwrap(),
        })
    }
}

impl<'a> QuoteSocket<'a> {
    pub fn new<CallbackFn>(server: DataServer, handler: CallbackFn) -> QuoteSocketBuilder<'a>
    where
        CallbackFn: FnMut(QuoteEvent, Value) -> Result<()> + 'a,
    {
        QuoteSocketBuilder {
            server,
            auth_token: None,
            quote_fields: None,
            handler: Some(Box::new(handler)),
        }
    }

    pub async fn quote_add_symbols(&mut self, symbols: Vec<String>) -> Result<()> {
        let mut payloads = payload![self.quote_session_id.clone()];
        payloads.extend(symbols.into_iter().map(Value::from));
        self.send("quote_add_symbols", payloads).await?;
        Ok(())
    }

    pub async fn quote_fast_symbols(&mut self, symbols: Vec<String>) -> Result<()> {
        let mut payloads = payload![self.quote_session_id.clone()];
        payloads.extend(symbols.into_iter().map(Value::from));
        self.send("quote_fast_symbols", payloads).await?;
        Ok(())
    }

    pub async fn quote_remove_symbols(&mut self, symbols: Vec<String>) -> Result<()> {
        let mut payloads = payload![self.quote_session_id.clone()];
        payloads.extend(symbols.into_iter().map(Value::from));
        self.send("quote_remove_symbols", payloads).await?;
        Ok(())
    }

    async fn handle_message(&mut self, message: Value) -> Result<()> {
        if let Some(message_type) = message.get(MESSAGE_TYPE_KEY).and_then(|m| m.as_str()) {
            match message_type {
                DATA_EVENT => {
                    if let Some(payload) = message.get("p").and_then(|p| p.get(PAYLOAD_KEY)) {
                        self.handle_data_event(Cow::Borrowed(payload))?;
                    }
                }
                LOADED_EVENT => {
                    self.handle_loaded_event(Cow::Borrowed(&message))?;
                }
                ERROR_EVENT => {
                    self.handle_error_event(Cow::Borrowed(&message))?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn handle_data_event(&mut self, payload: Cow<'_, Value>) -> Result<()> {
        if let Some(status) = payload.get(STATUS_KEY).and_then(|s| s.as_str()) {
            match status {
                OK_STATUS => {
                    (self.handler)(QuoteEvent::Data, payload.clone().into_owned())?;
                    return Ok(());
                }
                ERROR_STATUS => {
                    error!("failed to receive quote data: {:?}", payload);
                    (self.handler)(
                        QuoteEvent::Error(TradingViewError::QuoteDataError),
                        payload.into_owned(),
                    )?;
                    return Ok(());
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn handle_loaded_event(&mut self, message: Cow<'_, Value>) -> Result<()> {
        (self.handler)(QuoteEvent::Loaded, message.into_owned())?;
        Ok(())
    }

    fn handle_error_event(&mut self, message: Cow<'_, Value>) -> Result<()> {
        (self.handler)(
            QuoteEvent::Error(TradingViewError::CriticalError),
            message.into_owned(),
        )?;
        Ok(())
    }

    pub async fn event_loop(&mut self) {
        while let Some(result) = self.read.next().await {
            match result {
                Ok(message) => {
                    let message_str = match &message {
                        Message::Text(text) => text,
                        _ => {
                            warn!("Received non-text message: {:?}", message);
                            continue;
                        }
                    };
                    if let Ok(values) = parse_packet(message_str) {
                        for value in values {
                            match value {
                                Value::Number(_) => self.handle_ping(&message).await,
                                Value::Object(_) => {
                                    if let Err(e) = self.handle_message(value).await {
                                        error!("Error handling message: {:#?}", e);
                                    }
                                }
                                _ => (),
                            }
                        }
                    } else {
                        error!("Error parsing message: {:?}", message);
                    }
                }
                Err(e) => {
                    error!("Error reading message: {:#?}", e);
                }
            }
        }
    }

    async fn handle_ping(&mut self, message: &Message) {
        match self.ping(message).await {
            Ok(()) => debug!("ping sent"),
            Err(e) => {
                warn!("ping failed with: {:#?}", e);
            }
        }
    }

    async fn send<M, P>(&mut self, message: M, payload: Vec<P>) -> Result<()>
    where
        M: Serialize,
        P: Serialize,
    {
        let msg = format_packet(SocketMessage::new(message, payload))?;
        self.messages.push_back(msg);
        self.send_queue().await?;
        Ok(())
    }

    async fn send_queue(&mut self) -> Result<()> {
        while let Some(msg) = self.messages.pop_front() {
            self.write.send(msg).await?;
        }
        Ok(())
    }

    async fn ping(&mut self, ping: &Message) -> Result<()> {
        self.write.send(ping.clone()).await?;
        Ok(())
    }

    pub async fn delete_session(&mut self) -> Result<()> {
        self.send(
            "quote_delete_session",
            payload!(self.quote_session_id.clone()),
        )
        .await?;
        Ok(())
    }

    pub async fn update_session(&mut self) -> Result<()> {
        self.delete_session().await?;
        self.quote_session_id = gen_session_id("qs");
        self.send(
            "quote_create_session",
            payload!(self.quote_session_id.clone()),
        )
        .await?;
        Ok(())
    }

    pub async fn quote_set_fields(&mut self, fields: Vec<String>) -> Result<()> {
        let mut payload = vec![self.quote_session_id.clone()];
        payload.extend(fields);
        self.send("quote_set_fields", payload!(payload)).await?;
        Ok(())
    }

    pub async fn set_auth_token(&mut self, auth_token: &str) -> Result<()> {
        self.auth_token = auth_token.to_string();
        self.send("set_auth_token", payload!(self.auth_token.clone()))
            .await?;
        Ok(())
    }
}
