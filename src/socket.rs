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

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SocketServerInfo {
    #[serde(rename = "session_id")]
    pub session_id: String,
    pub timestamp: i64,
    pub timestamp_ms: i64,
    pub release: String,
    #[serde(rename = "studies_metadata_hash")]
    pub studies_metadata_hash: String,
    #[serde(rename = "auth_scheme_vsn")]
    pub auth_scheme_vsn: i64,
    pub protocol: String,
    pub via: String,
    pub javastudies: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum SocketMessageType<T> {
    SocketServerInfo(SocketServerInfo),
    SocketMessage(T),
}

pub enum DataServer {
    Data,
    ProData,
    WidgetData,
    MobileData,
}

impl std::fmt::Display for DataServer {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            DataServer::Data => write!(f, "data"),
            DataServer::ProData => write!(f, "prodata"),
            DataServer::WidgetData => write!(f, "widgetdata"),
            DataServer::MobileData => write!(f, "mobile-data"),
        }
    }
}

#[async_trait]
pub trait Socket {
    async fn connect(&mut self);
    async fn send(&mut self, m: String, p: Vec<Value>) -> Result<()>;
    async fn ping(&mut self, ping: &Message) -> Result<()>;
    async fn handle_message(&mut self, message: Value) -> Result<()>;
    async fn handle_error(&mut self, message: Value) -> Result<()>;
    async fn handle_data(&mut self, payload: Value) -> Result<()>;
    async fn handle_loaded(&mut self, payload: Value) -> Result<()>;
}
