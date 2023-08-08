use crate::{payload, prelude::*, utils::format_packet, WEBSOCKET_HEADERS};
use async_trait::async_trait;
use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::{
    net::TcpStream,
    time::{timeout, Duration},
};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, protocol::Message},
    MaybeTlsStream, WebSocketStream,
};
use url::Url;

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
    async fn connect(
        &mut self,
        server: DataServer,
        auth_token: Option<String>,
    ) -> Result<(
        SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>,
        SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    )> {
        let url = Url::parse(&format!(
            "wss://{server}.tradingview.com/socket.io/websocket",
            server = server.to_string()
        ))
        .unwrap();

        let mut request = url.into_client_request().unwrap();
        request.headers_mut().extend(WEBSOCKET_HEADERS.clone());

        let (socket, _response) = connect_async(request).await?;

        let (mut write, read) = socket.split();

        write
            .send(
                SocketMessage::new(
                    "set_auth_token",
                    payload!(auth_token.unwrap_or("unauthorized_user_token".to_string())),
                )
                .to_message()?,
            )
            .await?;

        Ok((write, read))
    }

    async fn send(
        mut write: SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>,
        m: String,
        p: Vec<Value>,
    ) -> Result<()> {
        let msg = SocketMessage::new(m, p).to_message()?;
        write.send(msg).await?;
        Ok(())
    }

    async fn ping(
        mut write: SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>,
        ping: &Message,
    ) -> Result<()> {
        write.send(ping.clone()).await?;
        Ok(())
    }

    async fn handle_message(&mut self, message: Value) -> Result<()>;
    async fn handle_error(&mut self, message: Value) -> Result<()>;
    async fn handle_data(&mut self, payload: Value) -> Result<()>;
    async fn handle_loaded(&mut self, payload: Value) -> Result<()>;
}
