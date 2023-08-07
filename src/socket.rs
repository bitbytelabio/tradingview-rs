use crate::{prelude::*, utils::format_packet};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio_tungstenite::tungstenite::protocol::Message;

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct SocketMessage {
    pub m: Value,
    pub p: Value,
}

impl SocketMessage {
    pub fn new<M, P>(m: M, p: Vec<P>) -> Self
    where
        M: Serialize,
        P: Serialize,
    {
        let m = serde_json::to_value(m).expect("Failed to serialize Socket Message");
        let p = serde_json::to_value(p).expect("Failed to serialize Socket Message");
        SocketMessage { m, p }
    }

    pub fn to_message(&self) -> Result<Message> {
        let msg = format_packet(self)?;
        Ok(msg)
    }
}

pub enum DataServer {
    Data,
    ProData,
    WidgetData,
}

impl std::fmt::Display for DataServer {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            DataServer::Data => write!(f, "data"),
            DataServer::ProData => write!(f, "prodata"),
            DataServer::WidgetData => write!(f, "widgetdata"),
        }
    }
}

#[async_trait]
trait Socket {
    async fn connect(&mut self);

    async fn send<M, P>(&mut self, m: M, p: Vec<P>) -> Result<()>
    where
        M: Serialize,
        P: Serialize;
    async fn send_queue(&mut self) -> Result<()>;

    async fn handle_ping(&mut self, message: &Message);
    async fn ping(&mut self, ping: &Message) -> Result<()>;

    async fn handle_message(&mut self, message: Value) -> Result<()>;
    async fn handle_error(&mut self, message: Value) -> Result<()>;
    async fn handle_data(&mut self, payload: Value) -> Result<()>;
    async fn handle_loaded(&mut self, payload: Value) -> Result<()>;
}
