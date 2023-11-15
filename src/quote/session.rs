use crate::{
    payload,
    quote::ALL_QUOTE_FIELDS,
    socket::{ Socket, SocketMessageDe, SocketSession, TradingViewDataEvent },
    utils::gen_session_id,
    Result,
    subscriber::Subscriber,
};
use async_trait::async_trait;
use serde_json::Value;

pub struct WebSocket {
    subscriber: Subscriber,
    socket: SocketSession,
    quote_session: String,
    quote_fields: Vec<Value>,
}

impl WebSocket {
    pub fn new(subscriber: Subscriber, socket: SocketSession) -> Self {
        let quote_session = gen_session_id("qs");

        let mut quote_fields = payload![quote_session.clone().to_string()];
        quote_fields.extend(ALL_QUOTE_FIELDS.clone().into_iter().map(Value::from));

        Self {
            subscriber,
            socket,
            quote_session,
            quote_fields,
        }
    }

    pub async fn create_session(&mut self) -> Result<()> {
        self.socket.send("quote_create_session", &payload!(self.quote_session.clone())).await?;
        Ok(())
    }

    pub async fn delete_session(&mut self) -> Result<()> {
        self.socket.send("quote_delete_session", &payload!(self.quote_session.clone())).await?;
        Ok(())
    }

    pub async fn update_session(&mut self) -> Result<()> {
        self.delete_session().await?;
        self.quote_session = gen_session_id("qs");
        self.socket.send("quote_create_session", &payload!(self.quote_session.clone())).await?;
        Ok(())
    }

    pub async fn set_fields(&mut self) -> Result<()> {
        self.socket.send("quote_set_fields", &self.quote_fields.clone()).await?;
        Ok(())
    }

    pub async fn add_symbols(&mut self, symbols: Vec<&str>) -> Result<()> {
        let mut payloads = payload![self.quote_session.clone()];
        payloads.extend(symbols.into_iter().map(Value::from));
        self.socket.send("quote_add_symbols", &payloads).await?;
        Ok(())
    }

    pub async fn update_auth_token(&mut self, auth_token: &str) -> Result<()> {
        self.socket.update_auth_token(auth_token).await?;
        Ok(())
    }

    pub async fn fast_symbols(&mut self, symbols: Vec<&str>) -> Result<()> {
        let mut payloads = payload![self.quote_session.clone()];
        payloads.extend(symbols.into_iter().map(Value::from));
        self.socket.send("quote_fast_symbols", &payloads).await?;
        Ok(())
    }

    pub async fn remove_symbols(&mut self, symbols: Vec<&str>) -> Result<()> {
        let mut payloads = payload![self.quote_session.clone()];
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
        let event = TradingViewDataEvent::from(message.m.clone());
        self.subscriber.handle_events(event, &message.p).await;
        Ok(())
    }
}
