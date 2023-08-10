use crate::{payload, prelude::*, utils::format_packet, UA};
use async_trait::async_trait;
use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use reqwest::header::{HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::net::TcpStream;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, protocol::Message},
    MaybeTlsStream, WebSocketStream,
};
use url::Url;

lazy_static::lazy_static! {
    pub static ref WEBSOCKET_HEADERS: HeaderMap<HeaderValue> = {
        let mut headers = HeaderMap::new();
        headers.insert("Origin", "https://www.tradingview.com/".parse().unwrap());
        headers.insert("User-Agent", UA.parse().unwrap());
        headers
    };
}

pub const ON_CHART_DATA: &str = "timescale_update";
pub const ON_CHART_DATA_UPDATE: &str = "du";
pub const ON_QUOTE_DATA: &str = "qsd";
pub const ON_QUOTE_COMPLETED: &str = "quote_completed";
pub const ON_SERIES_LOADING: &str = "series_loading";
pub const ON_SERIES_COMPLETED: &str = "series_completed";
pub const ON_SYMBOL_RESOLVED: &str = "symbol_resolved";
pub const ON_REPLAY_POINT: &str = "replay_point";
pub const ON_REPLAY_INSTANCE_ID: &str = "replay_instance_id";
pub const ON_REPLAY_RESOLUTIONS: &str = "replay_resolutions";
pub const ON_REPLAY_DATA_END: &str = "replay_data_end";
pub const ON_STUDY_COMPLETED: &str = "study_completed";
pub const ON_SYMBOL_ERROR: &str = "symbol_error";
pub const ON_SERIES_ERROR: &str = "series_error";
pub const ON_CRITICAL_ERROR: &str = "critical_error";

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct SocketMessage {
    pub m: Value,
    pub p: Value,
}

impl SocketMessage {
    pub fn new<M, P>(m: M, p: P) -> Self
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

#[derive(Default, Clone)]
pub enum DataServer {
    #[default]
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

pub enum ConnectionStatus {
    Connected,
    Disconnected,
    Error,
    Connecting,
}

#[async_trait]
pub trait Socket {
    async fn connect(
        server: &DataServer,
        auth_token: &str,
    ) -> Result<(
        SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>,
        SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    )> {
        let url = Url::parse(&format!(
            "wss://{}.tradingview.com/socket.io/websocket",
            server
        ))?;

        let mut request = url.into_client_request()?;
        request.headers_mut().extend(WEBSOCKET_HEADERS.clone());

        let (socket, _response) = connect_async(request).await?;

        let (mut write, read) = socket.split();

        write
            .send(SocketMessage::new("set_auth_token", payload!(auth_token)).to_message()?)
            .await?;

        Ok((write, read))
    }
    async fn send(&mut self, m: &str, p: &[Value]) -> Result<()>;
    async fn ping(&mut self, ping: &Message) -> Result<()>;
    async fn close(&mut self) -> Result<()>;
    async fn event_loop(&mut self);
    async fn handle_message(&mut self, message: Value) -> Result<()>;
}
