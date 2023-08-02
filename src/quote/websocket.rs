use crate::{
    prelude::*,
    quote::QuoteEvent,
    socket::{DataServer, SocketMessage},
    utils::{format_packet, gen_session_id, parse_packet},
    UA,
};
use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use serde::Serialize;
use serde_json::Value as JsonValue;

use std::borrow::Cow;
use std::collections::VecDeque;

use tokio::net::TcpStream;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, protocol::Message},
    MaybeTlsStream, WebSocketStream,
};

use tracing::{debug, error, info};
use url::Url;

pub struct QuoteSocket<'a> {
    write: SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>,
    read: SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    quote_session_id: String,
    messages: VecDeque<Message>,
    auth_token: String,
    handler: Box<dyn FnMut(QuoteEvent, JsonValue) -> Result<()> + 'a>,
}
pub struct QuoteSocketBuilder<'a> {
    server: DataServer,
    auth_token: Option<String>,
    quote_fields: Option<Vec<String>>,
    handler: Option<Box<dyn FnMut(QuoteEvent, JsonValue) -> Result<()> + 'a>>,
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
        let quote_fields: Vec<String> = match self.quote_fields.clone() {
            Some(fields) => {
                let mut quote_fields = vec![session.clone().to_string()];
                quote_fields.extend(fields);
                quote_fields
            }
            None => {
                let mut quote_fields = vec![session.clone().to_string()];
                quote_fields.extend(crate::quote::ALL_QUOTE_FIELDS.clone());
                quote_fields
            }
        };

        Ok(VecDeque::from(vec![
            SocketMessage::new("set_auth_token", &[auth_token]).to_message()?,
            SocketMessage::new("quote_create_session", &[session]).to_message()?,
            SocketMessage::new("quote_set_fields", &quote_fields).to_message()?,
        ]))
    }

    pub async fn build(&mut self) -> Result<QuoteSocket> {
        let url = Url::parse(&format!(
            "wss://{server}.tradingview.com/socket.io/websocket",
            server = self.server
        ))
        .unwrap();

        let mut request = url.into_client_request().unwrap();
        let headers = request.headers_mut();
        headers.insert("Origin", "https://www.tradingview.com/".parse().unwrap());
        headers.insert("User-Agent", UA.parse().unwrap());

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
        CallbackFn: FnMut(QuoteEvent, JsonValue) -> Result<()> + 'a,
    {
        QuoteSocketBuilder {
            server,
            auth_token: None,
            quote_fields: None,
            handler: Some(Box::new(handler)),
        }
    }

    pub async fn quote_add_symbols(&mut self, symbols: Vec<String>) -> Result<()> {
        let mut payload = vec![self.quote_session_id.clone()];
        payload.extend(symbols);
        self.send("quote_add_symbols", &payload).await?;
        Ok(())
    }

    pub async fn quote_fast_symbols(&mut self, symbols: Vec<String>) -> Result<()> {
        let mut payload = vec![self.quote_session_id.clone()];
        payload.extend(symbols);
        self.send("quote_fast_symbols", &payload).await?;
        Ok(())
    }

    pub async fn quote_remove_symbols(&mut self, symbols: Vec<String>) -> Result<()> {
        let mut payload = vec![self.quote_session_id.clone()];
        payload.extend(symbols);
        self.send("quote_remove_symbols", &payload).await?;
        Ok(())
    }

    async fn handle_msg(&mut self, message: JsonValue) -> Result<()> {
        const MESSAGE_TYPE_KEY: &str = "m";
        const PAYLOAD_KEY: usize = 1;
        const STATUS_KEY: &str = "s";
        const OK_STATUS: &str = "ok";
        const ERROR_STATUS: &str = "error";
        const DATA_EVENT: &str = "qsd";
        const LOADED_EVENT: &str = "quote_completed";
        const ERROR_EVENT: &str = "critical_error";

        let message: JsonValue = serde_json::from_value(message)?;

        let message_type = message
            .get(MESSAGE_TYPE_KEY)
            .and_then(|m| m.as_str().map(Cow::Borrowed));

        match message_type.as_ref().map(|s| s.as_ref()) {
            Some(DATA_EVENT) => {
                let payload = message.get("p").and_then(|p| p.get(PAYLOAD_KEY));
                let status = payload.and_then(|p| {
                    p.get(STATUS_KEY)
                        .and_then(|s| s.as_str().map(Cow::Borrowed))
                });

                match status.as_ref().map(|s| s.as_ref()) {
                    Some(OK_STATUS) => {
                        (self.handler)(QuoteEvent::Data, payload.unwrap().clone())?;
                        return Ok(());
                    }
                    Some(ERROR_STATUS) => {
                        error!("failed to receive quote data: {:?}", payload.unwrap());
                        (self.handler)(QuoteEvent::Error, message.clone())?;
                        return Ok(());
                    }
                    _ => {}
                }
            }
            Some(LOADED_EVENT) => {
                (self.handler)(QuoteEvent::Loaded, message.clone())?;
                return Ok(());
            }
            Some(ERROR_EVENT) => {
                (self.handler)(QuoteEvent::Error, message.clone())?;
                return Ok(());
            }
            _ => {}
        }

        Ok(())
    }

    pub async fn event_loop(&mut self) {
        while let Some(result) = self.read.next().await {
            match result {
                Ok(message) => {
                    let values = parse_packet(&message.to_string()).unwrap();
                    for value in values {
                        match value {
                            JsonValue::Number(_) => self.ping(&message).await.unwrap(),
                            JsonValue::Object(_) => self.handle_msg(value).await.unwrap(),
                            _ => (),
                        }
                    }
                }
                Err(e) => {
                    error!("Error reading message: {:#?}", e);
                }
            }
        }
    }

    async fn send<M, P>(&mut self, message: M, payload: &[P]) -> Result<()>
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
        while !self.messages.is_empty() {
            let msg = self.messages.pop_front().unwrap();
            self.write.send(msg).await?;
        }
        Ok(())
    }

    async fn ping(&mut self, ping: &Message) -> Result<()> {
        self.write.send(ping.clone()).await?;
        Ok(())
    }

    pub async fn delete_session(&mut self) -> Result<()> {
        self.send("quote_delete_session", &[self.quote_session_id.clone()])
            .await?;
        Ok(())
    }

    pub async fn update_session(&mut self) -> Result<()> {
        self.delete_session().await?;
        self.quote_session_id = gen_session_id("qs");
        self.send("quote_create_session", &[self.quote_session_id.clone()])
            .await?;
        Ok(())
    }

    pub async fn quote_set_fields(&mut self, fields: Vec<String>) -> Result<()> {
        let mut payload = vec![self.quote_session_id.clone()];
        payload.extend(fields);
        self.send("quote_set_fields", &payload).await?;
        Ok(())
    }

    pub async fn set_auth_token(&mut self, auth_token: &str) -> Result<()> {
        self.auth_token = auth_token.to_string();
        self.send("set_auth_token", &[self.auth_token.clone()])
            .await?;
        Ok(())
    }
}
