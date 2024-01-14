use crate::{
    client::websocket::WSClient,
    payload,
    quote::ALL_QUOTE_FIELDS,
    socket::{Socket, SocketMessageDe, SocketSession, TradingViewDataEvent},
    utils::gen_session_id,
    Error, Result,
};
use serde_json::Value;

#[derive(Clone)]
pub struct WebSocket<'a> {
    pub(crate) client: WSClient<'a>,
    socket: SocketSession,
}

impl<'a> WebSocket<'a> {
    pub fn new(client: WSClient<'a>, socket: SocketSession) -> Self {
        Self { client, socket }
    }

    pub async fn create_session(&mut self) -> Result<&mut Self> {
        let quote_session = gen_session_id("qs");
        self.client.metadata.quote_session = quote_session.clone();
        self.socket
            .send("quote_create_session", &payload!(quote_session))
            .await?;
        Ok(self)
    }

    pub async fn delete_session(&mut self) -> Result<&mut Self> {
        self.socket
            .send(
                "quote_delete_session",
                &payload!(self.client.metadata.quote_session.clone()),
            )
            .await?;
        Ok(self)
    }

    pub async fn set_fields(&mut self) -> Result<&mut Self> {
        let mut quote_fields = payload![self.client.metadata.quote_session.clone().to_string()];
        quote_fields.extend(ALL_QUOTE_FIELDS.clone().into_iter().map(Value::from));
        self.socket.send("quote_set_fields", &quote_fields).await?;
        Ok(self)
    }

    pub async fn add_symbols(&mut self, symbols: Vec<&str>) -> Result<&mut Self> {
        let mut payloads = payload![self.client.metadata.quote_session.clone()];
        payloads.extend(symbols.into_iter().map(Value::from));
        self.socket.send("quote_add_symbols", &payloads).await?;
        Ok(self)
    }

    pub async fn update_auth_token(&mut self, auth_token: &str) -> Result<&mut Self> {
        self.socket.update_auth_token(auth_token).await?;
        Ok(self)
    }

    pub async fn fast_symbols(&mut self, symbols: Vec<&str>) -> Result<&mut Self> {
        let mut payloads = payload![self.client.metadata.quote_session.clone()];
        payloads.extend(symbols.into_iter().map(Value::from));
        self.socket.send("quote_fast_symbols", &payloads).await?;
        Ok(self)
    }

    pub async fn remove_symbols(&mut self, symbols: Vec<&str>) -> Result<&mut Self> {
        let mut payloads = payload![self.client.metadata.quote_session.clone()];
        payloads.extend(symbols.into_iter().map(Value::from));
        self.socket.send("quote_remove_symbols", &payloads).await?;
        Ok(self)
    }

    pub async fn subscribe(&mut self) {
        self.event_loop(&mut self.socket.to_owned()).await;
    }
}

#[async_trait::async_trait]
impl<'a> Socket for WebSocket<'a> {
    async fn handle_message_data(&mut self, message: SocketMessageDe) -> Result<()> {
        let event = TradingViewDataEvent::from(message.m.to_owned());
        self.client.handle_events(event, &message.p).await;
        Ok(())
    }

    async fn handle_error(&self, error: Error) {
        (self.client.callbacks.on_error)(error).await;
    }
}
