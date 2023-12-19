use crate::{
    feeder::Feeder,
    payload,
    quote::ALL_QUOTE_FIELDS,
    socket::{Socket, SocketMessageDe, SocketSession, TradingViewDataEvent},
    utils::gen_session_id,
    Result,
};
use async_trait::async_trait;
use serde_json::Value;

#[derive(Clone)]
pub struct WebSocket {
    feeder: Feeder,
    socket: SocketSession,
}

impl WebSocket {
    pub fn new(feeder: Feeder, socket: SocketSession) -> Self {
        Self { feeder, socket }
    }

    pub async fn create_session(&mut self) -> Result<&mut Self> {
        let quote_session = gen_session_id("qs");
        self.feeder.metadata.quote_session = quote_session.clone();
        self.socket
            .send("quote_create_session", &payload!(quote_session))
            .await?;
        Ok(self)
    }

    pub async fn delete_session(&mut self) -> Result<&mut Self> {
        self.socket
            .send(
                "quote_delete_session",
                &payload!(self.feeder.metadata.quote_session.clone()),
            )
            .await?;
        Ok(self)
    }

    pub async fn set_fields(&mut self) -> Result<&mut Self> {
        let mut quote_fields = payload![self.feeder.metadata.quote_session.clone().to_string()];
        quote_fields.extend(ALL_QUOTE_FIELDS.clone().into_iter().map(Value::from));
        self.socket.send("quote_set_fields", &quote_fields).await?;
        Ok(self)
    }

    pub async fn add_symbols(&mut self, symbols: Vec<&str>) -> Result<&mut Self> {
        let mut payloads = payload![self.feeder.metadata.quote_session.clone()];
        payloads.extend(symbols.into_iter().map(Value::from));
        self.socket.send("quote_add_symbols", &payloads).await?;
        Ok(self)
    }

    pub async fn update_auth_token(&mut self, auth_token: &str) -> Result<&mut Self> {
        self.socket.update_auth_token(auth_token).await?;
        Ok(self)
    }

    pub async fn fast_symbols(&mut self, symbols: Vec<&str>) -> Result<&mut Self> {
        let mut payloads = payload![self.feeder.metadata.quote_session.clone()];
        payloads.extend(symbols.into_iter().map(Value::from));
        self.socket.send("quote_fast_symbols", &payloads).await?;
        Ok(self)
    }

    pub async fn remove_symbols(&mut self, symbols: Vec<&str>) -> Result<&mut Self> {
        let mut payloads = payload![self.feeder.metadata.quote_session.clone()];
        payloads.extend(symbols.into_iter().map(Value::from));
        self.socket.send("quote_remove_symbols", &payloads).await?;
        Ok(self)
    }

    pub async fn subscribe(&mut self) {
        self.event_loop(&mut self.socket.clone()).await;
    }
}

#[async_trait]
impl Socket for WebSocket {
    async fn handle_message_data(&mut self, message: SocketMessageDe) -> Result<()> {
        let event = TradingViewDataEvent::from(message.m.clone());
        self.feeder.handle_events(event, &message.p).await;
        Ok(())
    }
}
