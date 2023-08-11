use crate::{
    error::TradingViewError,
    payload,
    prelude::*,
    quote::{QuoteData, ALL_QUOTE_FIELDS},
    socket::{DataServer, Socket, SocketEvent, SocketMessageDe, SocketMessageSer},
    utils::gen_session_id,
};
use async_trait::async_trait;
use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use serde_json::Value;
use std::{collections::HashMap, rc::Rc, sync::Arc};
use tokio::net::TcpStream;
use tokio::sync::RwLock;
use tokio_tungstenite::{tungstenite::protocol::Message, MaybeTlsStream, WebSocketStream};
use tracing::{debug, error, info, trace, warn};

use super::QuoteValue;

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
    prev_quotes: HashMap<String, QuoteValue>,
    auth_token: String,
    callbacks: QuoteCallbackFn,
}

fn merge_quotes(quote_old: &QuoteValue, quote_new: &QuoteValue) -> QuoteValue {
    QuoteValue {
        ask: quote_new.ask.or(quote_old.ask),
        ask_size: quote_new.ask_size.or(quote_old.ask_size),
        bid: quote_new.bid.or(quote_old.bid),
        bid_size: quote_new.bid_size.or(quote_old.bid_size),
        price_change: quote_new.price_change.or(quote_old.price_change),
        price_change_percent: quote_new
            .price_change_percent
            .or(quote_old.price_change_percent),
        open: quote_new.open.or(quote_old.open),
        high: quote_new.high.or(quote_old.high),
        low: quote_new.low.or(quote_old.low),
        prev_close: quote_new.prev_close.or(quote_old.prev_close),
        price: quote_new.price.or(quote_old.price),
        timestamp: quote_new.timestamp.or(quote_old.timestamp),
        volume: quote_new.volume.or(quote_old.volume),
    }
}

pub struct QuoteCallbackFn {
    pub data: Box<dyn FnMut(HashMap<String, QuoteValue>) -> Result<()> + Send + Sync>,
    pub loaded: Box<dyn FnMut(Vec<Value>) -> Result<()> + Send + Sync>,
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
            prev_quotes: HashMap::new(),
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
                Some(Ok(message)) => self.handle_raw_messages(message).await,
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
            SocketEvent::OnQuoteData => {
                trace!("received OnQuoteData: {:#?}", message);
                let qsd = serde_json::from_value::<QuoteData>(message.p[1].clone())?;
                if qsd.status == "ok" {
                    if let Some(prev_quote) = self.prev_quotes.get_mut(&qsd.name) {
                        *prev_quote = merge_quotes(prev_quote, &qsd.value);
                    } else {
                        self.prev_quotes.insert(qsd.name.clone(), qsd.value.clone());
                    }
                    (self.callbacks.data)(self.prev_quotes.clone())?;
                    return Ok(());
                } else {
                    (self.callbacks.error)(TradingViewError::QuoteDataStatusError)?;
                    return Ok(());
                }
            }
            SocketEvent::OnQuoteCompleted => {
                (self.callbacks.loaded)(message.p.clone())?;
                return Ok(());
            }
            SocketEvent::OnError(e) => {
                (self.callbacks.error)(e)?;
                return Ok(());
            }
            _ => {
                trace!("received message: {:#?}", message);
            }
        };
        Ok(())
    }
}
