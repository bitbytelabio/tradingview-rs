use crate::{
    error::TradingViewError,
    payload,
    prelude::*,
    utils::{format_packet, parse_packet},
    UA,
};
use async_trait::async_trait;
use futures_util::{
    future::BoxFuture,
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use reqwest::header::{HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tokio::{net::TcpStream, sync::Mutex};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, protocol::Message},
    MaybeTlsStream, WebSocketStream,
};
use tracing::{debug, error, info, trace, warn};
use url::Url;

pub(crate) type AsyncCallback<T> = Box<dyn Fn(T) -> BoxFuture<'static, Result<()>> + Send + Sync>;

lazy_static::lazy_static! {
    pub static ref WEBSOCKET_HEADERS: HeaderMap<HeaderValue> = {
        let mut headers = HeaderMap::new();
        headers.insert("Origin", "https://www.tradingview.com/".parse().unwrap());
        headers.insert("User-Agent", UA.parse().unwrap());
        headers
    };
}

pub enum TradingViewDataEvent {
    OnChartData,
    OnChartDataUpdate,
    OnQuoteData,
    OnQuoteCompleted,
    OnSeriesLoading,
    OnSeriesCompleted,
    OnSymbolResolved,
    OnReplayOk,
    OnReplayPoint,
    OnReplayInstanceId,
    OnReplayResolutions,
    OnReplayDataEnd,
    OnStudyLoading,
    OnStudyCompleted,
    OnError(TradingViewError),
    UnknownEvent(String),
}

impl From<String> for TradingViewDataEvent {
    fn from(s: String) -> Self {
        match s.as_str() {
            "timescale_update" => TradingViewDataEvent::OnChartData,
            "du" => TradingViewDataEvent::OnChartDataUpdate,

            "qsd" => TradingViewDataEvent::OnQuoteData,
            "quote_completed" => TradingViewDataEvent::OnQuoteCompleted,

            "series_loading" => TradingViewDataEvent::OnSeriesLoading,
            "series_completed" => TradingViewDataEvent::OnSeriesCompleted,

            "symbol_resolved" => TradingViewDataEvent::OnSymbolResolved,

            "replay_ok" => TradingViewDataEvent::OnReplayOk,
            "replay_point" => TradingViewDataEvent::OnReplayPoint,
            "replay_instance_id" => TradingViewDataEvent::OnReplayInstanceId,
            "replay_resolutions" => TradingViewDataEvent::OnReplayResolutions,
            "replay_data_end" => TradingViewDataEvent::OnReplayDataEnd,

            "study_loading" => TradingViewDataEvent::OnSeriesLoading,
            "study_completed" => TradingViewDataEvent::OnStudyCompleted,

            "symbol_error" => TradingViewDataEvent::OnError(TradingViewError::SymbolError),
            "series_error" => TradingViewDataEvent::OnError(TradingViewError::SeriesError),
            "critical_error" => TradingViewDataEvent::OnError(TradingViewError::CriticalError),
            "study_error" => TradingViewDataEvent::OnError(TradingViewError::StudyError),
            s => TradingViewDataEvent::UnknownEvent(s.to_string()),
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
    pub t: Option<u64>,
    #[serde(default)]
    pub t_ms: Option<u64>,
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

pub(crate) enum _ConnectionStatus {
    Connected,
    Disconnected,
    Error,
    Connecting,
}

#[derive(Clone)]
pub struct SocketSession {
    read: Arc<Mutex<SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>>>,
    write: Arc<Mutex<SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>>>,
}

impl SocketSession {
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

    pub async fn new(server: DataServer, auth_token: String) -> Result<SocketSession> {
        let (write_stream, read_stream) = SocketSession::connect(&server, &auth_token).await?;

        let write = Arc::from(Mutex::new(write_stream));
        let read = Arc::from(Mutex::new(read_stream));

        Ok(SocketSession { write, read })
    }

    pub async fn send(&mut self, m: &str, p: &[Value]) -> Result<()> {
        self.write
            .lock()
            .await
            .send(SocketMessageSer::new(m, p).to_message()?)
            .await?;
        Ok(())
    }

    pub async fn ping(&mut self, ping: &Message) -> Result<()> {
        self.write.lock().await.send(ping.clone()).await?;
        Ok(())
    }

    pub async fn close(&mut self) -> Result<()> {
        self.write.lock().await.close().await?;
        Ok(())
    }
}

#[async_trait]
pub(crate) trait Socket {
    async fn event_loop(&mut self, session: &mut SocketSession) {
        let read = session.read.clone();
        let mut read_guard = read.lock().await;
        loop {
            trace!("waiting for next message");
            match read_guard.next().await {
                Some(Ok(message)) => self.handle_raw_messages(session, message).await,
                Some(Err(e)) => {
                    error!("Error reading message: {:#?}", e);
                    self.handle_error(Error::WebSocketError(e)).await;
                }
                None => {
                    debug!("no messages to read");
                    continue;
                }
            }
        }
    }

    async fn handle_raw_messages(&mut self, session: &mut SocketSession, raw: Message) {
        match &raw {
            Message::Text(text) => {
                trace!("parsing message: {:?}", text);
                match parse_packet(text) {
                    Ok(parsed_messages) => {
                        self.handle_parsed_messages(session, parsed_messages, &raw)
                            .await;
                    }
                    Err(e) => {
                        error!("error parsing message: {:?}", e);
                        self.handle_error(e).await;
                    }
                }
            }
            Message::Close(msg) => {
                warn!("connection closed with code: {:?}", msg);
            }
            Message::Binary(msg) => {
                debug!("received binary message: {:?}", msg);
            }
            Message::Ping(msg) => {
                trace!("received ping message: {:?}", msg);
            }
            Message::Pong(msg) => {
                trace!("received pong message: {:?}", msg);
            }
            Message::Frame(f) => {
                debug!("received frame message: {:?}", f);
            }
        }
    }

    async fn handle_parsed_messages(
        &mut self,
        session: &mut SocketSession,
        messages: Vec<SocketMessage<SocketMessageDe>>,
        raw: &Message,
    ) {
        for message in messages {
            match message {
                SocketMessage::SocketServerInfo(info) => {
                    info!("received server info: {:?}", info);
                }
                SocketMessage::SocketMessage(msg) => {
                    if let Err(e) = self.handle_message_data(msg).await {
                        self.handle_error(e).await;
                    }
                }
                SocketMessage::Other(value) => {
                    trace!("receive message: {:?}", value);
                    if value.is_number() {
                        trace!("handling ping message: {:?}", value);
                        if let Err(e) = session.ping(raw).await {
                            self.handle_error(e).await;
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

    async fn handle_message_data(&mut self, message: SocketMessageDe) -> Result<()>;

    async fn handle_error(&mut self, error: Error);
}
