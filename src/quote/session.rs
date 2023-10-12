use crate::{
    error::TradingViewError,
    payload,
    quote::{ QuoteData, QuoteValue, ALL_QUOTE_FIELDS },
    socket::{
        AsyncCallback,
        DataServer,
        Socket,
        SocketMessageDe,
        SocketSession,
        TradingViewDataEvent,
    },
    utils::gen_session_id,
    error::Error,
    Result,
};
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use tracing::{ debug, error, trace };

#[derive(Default)]
pub struct WebSocketsBuilder {
    server: Option<DataServer>,
    auth_token: Option<String>,
    quote_fields: Option<Vec<String>>,
    socket: Option<SocketSession>,
}

pub struct WebSocket {
    socket: SocketSession,
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
        change: quote_new.change.or(quote_old.change),
        change_percent: quote_new.change_percent.or(quote_old.change_percent),
        open: quote_new.open.or(quote_old.open),
        high: quote_new.high.or(quote_old.high),
        low: quote_new.low.or(quote_old.low),
        prev_close: quote_new.prev_close.or(quote_old.prev_close),
        price: quote_new.price.or(quote_old.price),
        timestamp: quote_new.timestamp.or(quote_old.timestamp),
        volume: quote_new.volume.or(quote_old.volume),
        description: quote_new.description.clone().or(quote_old.description.clone()),
        country: quote_new.country.clone().or(quote_old.country.clone()),
        currency: quote_new.currency.clone().or(quote_old.currency.clone()),
        data_provider: quote_new.data_provider.clone().or(quote_old.data_provider.clone()),
        symbol: quote_new.symbol.clone().or(quote_old.symbol.clone()),
        symbol_id: quote_new.symbol_id.clone().or(quote_old.symbol_id.clone()),
        exchange: quote_new.exchange.clone().or(quote_old.exchange.clone()),
        market_type: quote_new.market_type.clone().or(quote_old.market_type.clone()),
    }
}

pub struct QuoteCallbackFn {
    pub data: AsyncCallback<HashMap<String, QuoteValue>>,
    pub loaded: AsyncCallback<Vec<Value>>,
    pub error: AsyncCallback<Error>,
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

    pub fn quote_fields(&mut self, quote_fields: Vec<String>) -> &mut Self {
        self.quote_fields = Some(quote_fields);
        self
    }

    pub async fn connect(&self, callback: QuoteCallbackFn) -> Result<WebSocket> {
        let auth_token = self.auth_token.clone().unwrap_or("unauthorized_user_token".to_string());

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

        let socket = self.socket
            .clone()
            .unwrap_or(SocketSession::new(server, auth_token.clone()).await?);

        Ok(WebSocket {
            socket,
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
        self.socket.send("quote_create_session", &payload!(self.quote_session_id.clone())).await?;
        Ok(())
    }

    pub async fn delete_session(&mut self) -> Result<()> {
        self.socket.send("quote_delete_session", &payload!(self.quote_session_id.clone())).await?;
        Ok(())
    }

    pub async fn update_session(&mut self) -> Result<()> {
        self.delete_session().await?;
        self.quote_session_id = gen_session_id("qs");
        self.socket.send("quote_create_session", &payload!(self.quote_session_id.clone())).await?;
        Ok(())
    }

    pub async fn set_fields(&mut self) -> Result<()> {
        self.socket.send("quote_set_fields", &self.quote_fields.clone()).await?;
        Ok(())
    }

    pub async fn add_symbols(&mut self, symbols: Vec<&str>) -> Result<()> {
        let mut payloads = payload![self.quote_session_id.clone()];
        payloads.extend(symbols.into_iter().map(Value::from));
        self.socket.send("quote_add_symbols", &payloads).await?;
        Ok(())
    }

    pub async fn update_auth_token(&mut self, auth_token: &str) -> Result<()> {
        self.auth_token = auth_token.to_owned();
        self.socket.send("set_auth_token", &payload!(auth_token)).await?;
        Ok(())
    }

    pub async fn fast_symbols(&mut self, symbols: Vec<&str>) -> Result<()> {
        let mut payloads = payload![self.quote_session_id.clone()];
        payloads.extend(symbols.into_iter().map(Value::from));
        self.socket.send("quote_fast_symbols", &payloads).await?;
        Ok(())
    }

    pub async fn remove_symbols(&mut self, symbols: Vec<&str>) -> Result<()> {
        let mut payloads = payload![self.quote_session_id.clone()];
        payloads.extend(symbols.into_iter().map(Value::from));
        self.socket.send("quote_remove_symbols", &payloads).await?;
        Ok(())
    }

    pub async fn subscribe(&mut self) {
        self.event_loop(&mut self.socket.clone()).await;
    }
}

#[async_trait]
impl Socket for WebSocket {
    async fn handle_message_data(&mut self, message: SocketMessageDe) -> Result<()> {
        match TradingViewDataEvent::from(message.m.clone()) {
            TradingViewDataEvent::OnQuoteData => {
                trace!("received OnQuoteData: {:#?}", message);
                let qsd = serde_json::from_value::<QuoteData>(message.p[1].clone())?;
                if qsd.status == "ok" {
                    if let Some(prev_quote) = self.prev_quotes.get_mut(&qsd.name) {
                        *prev_quote = merge_quotes(prev_quote, &qsd.value);
                    } else {
                        self.prev_quotes.insert(qsd.name.clone(), qsd.value.clone());
                    }
                    (self.callbacks.data)(self.prev_quotes.clone()).await;
                    return Ok(());
                } else {
                    (self.callbacks.error)(
                        Error::TradingViewError(TradingViewError::QuoteDataStatusError)
                    ).await;
                    return Ok(());
                }
            }
            TradingViewDataEvent::OnQuoteCompleted => {
                (self.callbacks.loaded)(message.p.clone()).await;
            }
            TradingViewDataEvent::OnError(e) => {
                self.handle_error(Error::TradingViewError(e)).await;
            }
            _ => {
                debug!("unhandled event on this session: {:?}", message);
            }
        }
        Ok(())
    }

    async fn handle_error(&mut self, error: Error) {
        error!("error handling event: {:#?}", error);
    }
}
