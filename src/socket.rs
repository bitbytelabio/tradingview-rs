use crate::{
    error::TradingViewError,
    payload,
    prelude::*,
    utils::{format_packet, parse_packet},
    UA,
};
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
use tracing::{error, info, trace, warn};
use url::Url;

lazy_static::lazy_static! {
    pub static ref WEBSOCKET_HEADERS: HeaderMap<HeaderValue> = {
        let mut headers = HeaderMap::new();
        headers.insert("Origin", "https://www.tradingview.com/".parse().unwrap());
        headers.insert("User-Agent", UA.parse().unwrap());
        headers
    };
}

pub enum SocketEvent {
    OnChartData,
    OnChartDataUpdate,
    OnQuoteData,
    OnQuoteCompleted,
    OnSeriesLoading,
    OnSeriesCompleted,
    OnSymbolResolved,
    OnReplayPoint,
    OnReplayInstanceId,
    OnReplayResolutions,
    OnReplayDataEnd,
    OnStudyCompleted,
    OnError(TradingViewError),
    UnknownEvent(String),
}

impl From<String> for SocketEvent {
    fn from(s: String) -> Self {
        match s.as_str() {
            "timescale_update" => SocketEvent::OnChartData,
            "du" => SocketEvent::OnChartDataUpdate,
            "qsd" => SocketEvent::OnQuoteData,
            "quote_completed" => SocketEvent::OnQuoteCompleted,
            "series_loading" => SocketEvent::OnSeriesLoading,
            "series_completed" => SocketEvent::OnSeriesCompleted,
            "symbol_resolved" => SocketEvent::OnSymbolResolved,
            "replay_point" => SocketEvent::OnReplayPoint,
            "replay_instance_id" => SocketEvent::OnReplayInstanceId,
            "replay_resolutions" => SocketEvent::OnReplayResolutions,
            "replay_data_end" => SocketEvent::OnReplayDataEnd,
            "study_completed" => SocketEvent::OnStudyCompleted,
            "symbol_error" => SocketEvent::OnError(TradingViewError::SymbolError),
            "series_error" => SocketEvent::OnError(TradingViewError::SeriesError),
            "critical_error" => SocketEvent::OnError(TradingViewError::CriticalError),
            s => SocketEvent::UnknownEvent(s.to_string()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SocketMessageSer {
    pub m: Value,
    pub p: Value,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SocketMessageDe {
    pub m: String,
    pub p: Vec<Value>,
    #[serde(default)]
    pub t: Option<i64>,
    #[serde(default)]
    pub t_ms: Option<i64>,
}

impl SocketMessageSer {
    pub fn new<M, P>(m: M, p: P) -> Self
    where
        M: Serialize,
        P: Serialize,
    {
        let m = serde_json::to_value(m).expect("Failed to serialize Socket Message");
        let p = serde_json::to_value(p).expect("Failed to serialize Socket Message");
        SocketMessageSer { m, p }
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
pub enum SocketMessage<T> {
    SocketServerInfo(SocketServerInfo),
    SocketMessage(T),
    Other(Value),
    Unknown(String),
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

pub struct SocketSession {
    pub read: SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    pub write: SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>,
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
            .send(SocketMessageSer::new("set_auth_token", payload!(auth_token)).to_message()?)
            .await?;

        Ok((write, read))
    }
    async fn send(&mut self, m: &str, p: &[Value]) -> Result<()>;
    async fn ping(&mut self, ping: &Message) -> Result<()>;
    async fn close(&mut self) -> Result<()>;
    async fn event_loop(&mut self);
    async fn handle_raw_messages(&mut self, raw: Message) {
        match &raw {
            Message::Text(text) => {
                trace!("parsing message: {:?}", text);
                match parse_packet(text) {
                    Ok(parsed_messages) => {
                        self.handle_parsed_messages(parsed_messages, &raw).await;
                    }
                    Err(e) => {
                        error!("Error parsing message: {:#?}", e);
                    }
                }
            }
            _ => {
                warn!("received non-text message: {:?}", raw);
            }
        }
    }
    async fn handle_parsed_messages(
        &mut self,
        messages: Vec<SocketMessage<SocketMessageDe>>,
        raw: &Message,
    ) {
        for message in messages {
            match message {
                SocketMessage::SocketServerInfo(info) => {
                    info!("received server info: {:?}", info);
                }
                SocketMessage::SocketMessage(msg) => {
                    if let Err(e) = self.handle_event(msg).await {
                        error!("Error handling message: {:#?}", e);
                    }
                }
                SocketMessage::Other(value) => {
                    trace!("receive message: {:?}", value);
                    if value.is_number() {
                        trace!("handling ping message: {:?}", value);
                        if let Err(e) = self.ping(&raw).await {
                            error!("error handling ping: {:#?}", e);
                        }
                    } else {
                        warn!("unhandled message: {:?}", value)
                    }
                }
                SocketMessage::Unknown(s) => {
                    warn!("unknown message: {:?}", s);
                }
            }
        }
    }
    async fn handle_event(&mut self, message: SocketMessageDe) -> Result<()>;
}
