use crate::{
    Result, UA,
    error::Error,
    error::TradingViewError,
    payload,
    utils::{format_packet, parse_packet},
};
use futures_util::{
    SinkExt, StreamExt,
    stream::{SplitSink, SplitStream},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use tokio::{net::TcpStream, sync::RwLock};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async,
    tungstenite::{
        client::IntoClientRequest,
        http::{HeaderMap, HeaderValue},
        protocol::Message,
    },
};
use tracing::{debug, error, info, trace, warn};
use url::Url;

lazy_static::lazy_static! {
    pub static ref WEBSOCKET_HEADERS: HeaderMap<HeaderValue> = {
        let mut headers = HeaderMap::new();
        headers.insert("Origin", "https://www.tradingview.com/".parse().unwrap());
        headers.insert("User-Agent", UA.parse().unwrap());
        headers
    };
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
            "protocol_error" => TradingViewDataEvent::OnError(TradingViewError::ProtocolError),
            "replay_error" => TradingViewDataEvent::OnError(TradingViewError::ReplayError),

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

#[derive(Default, Clone, Debug, PartialEq, Serialize, Deserialize, Copy, Eq)]
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

#[derive(Clone)]
pub struct SocketSession {
    server: Arc<DataServer>,
    auth_token: Arc<String>,
    read: Arc<RwLock<SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>>>,
    write: Arc<RwLock<SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>>>,
}

impl SocketSession {
    /// Establishes a WebSocket connection to a TradingView data server.
    ///
    /// # Arguments
    ///
    /// * `server` - A reference to the `DataServer` enum which represents the server to connect to.
    /// * `auth_token` - A string slice that holds the authentication token.
    ///
    /// # Returns
    ///
    /// * A `Result` which is:
    ///     * `Ok` - A tuple containing the split sink and stream of the WebSocket connection.
    ///     * `Err` - An error that occurred while trying to establish the connection or send the authentication message.
    ///
    /// # Asynchronous
    ///
    /// This function is asynchronous, it returns a Future that should be awaited.
    ///
    /// This function first constructs the URL for the WebSocket connection based on the provided server.
    /// It then creates a client request from the URL and adds necessary headers.
    /// The `connect_async` function is used to establish the WebSocket connection.
    /// The connection is then split into a write and read part.
    /// An authentication message is sent using the write part of the connection.
    /// Finally, it returns the write and read parts of the connection.
    async fn connect(
        server: &DataServer,
        auth_token: &str,
    ) -> Result<(
        SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>,
        SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    )> {
        let url = Url::parse(&format!(
            "wss://{server}.tradingview.com/socket.io/websocket"
        ))?;

        let mut request = url.into_client_request()?;
        request
            .headers_mut()
            .extend(WEBSOCKET_HEADERS.clone().into_iter());

        let (socket, _response) = connect_async(request).await?;

        let (mut write, read) = socket.split();

        write
            .send(SocketMessageSer::new("set_auth_token", payload!(auth_token)).to_message()?)
            .await?;

        Ok((write, read))
    }

    pub async fn reconnect(&mut self) -> Result<()> {
        let (write, read) = SocketSession::connect(
            self.server.clone().as_ref(),
            self.auth_token.clone().as_ref(),
        )
        .await?;
        self.write = Arc::from(RwLock::new(write));
        self.read = Arc::from(RwLock::new(read));
        Ok(())
    }

    pub async fn update_auth_token(&mut self, auth_token: &str) -> Result<()> {
        self.auth_token = Arc::new(auth_token.to_string());
        self.send("set_auth_token", &payload!(auth_token)).await?;
        Ok(())
    }

    pub async fn new(server: DataServer, auth_token: String) -> Result<SocketSession> {
        let (write_stream, read_stream) = SocketSession::connect(&server, &auth_token).await?;

        let write = Arc::from(RwLock::new(write_stream));
        let read = Arc::from(RwLock::new(read_stream));
        let server = Arc::new(server);
        let auth_token = Arc::new(auth_token);

        Ok(SocketSession {
            server,
            auth_token,
            write,
            read,
        })
    }

    pub async fn send(&self, m: &str, p: &[Value]) -> Result<()> {
        self.write
            .write()
            .await
            .send(SocketMessageSer::new(m, p).to_message()?)
            .await?;
        Ok(())
    }

    pub async fn ping(&self, ping: &Message) -> Result<()> {
        self.write.write().await.send(ping.clone()).await?;
        trace!("sent ping message {}", ping);
        Ok(())
    }

    pub async fn close(&self) -> Result<()> {
        self.write.write().await.close().await?;
        Ok(())
    }

    pub async fn update_token(&self, auth_token: &str) -> Result<()> {
        self.write
            .write()
            .await
            .send(SocketMessageSer::new("set_auth_token", payload!(auth_token)).to_message()?)
            .await?;
        Ok(())
    }
}

#[async_trait::async_trait]
pub trait Socket {
    async fn event_loop(&self, session: &SocketSession) {
        let read = session.read.clone();
        let mut read_guard = read.write().await;
        loop {
            trace!("waiting for next message");
            match read_guard.next().await {
                Some(Ok(message)) => self.handle_raw_messages(session, message).await,
                Some(Err(e)) => {
                    error!("Error reading message: {:#?}", e);
                    self.handle_error(Error::WebSocketError(Box::new(e))).await;
                }
                None => {
                    trace!("no messages to read");
                    continue;
                }
            }
        }
    }

    async fn handle_raw_messages(&self, session: &SocketSession, raw: Message) {
        match &raw {
            Message::Text(text) => {
                trace!("parsing message: {:?}", text);
                self.handle_parsed_messages(session, parse_packet(text), &raw)
                    .await;
            }
            Message::Close(msg) => {
                warn!("connection closed with code: {:?}", msg);
            }
            Message::Binary(msg) => {
                debug!("received binary message: {:?}", msg);
                // TODO: handle binary messages
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
        &self,
        session: &SocketSession,
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
                        warn!("unhandled message: {:?}", value);
                    }
                }
                SocketMessage::Unknown(s) => {
                    warn!("unknown message: {:?}", s);
                }
            }
        }
    }

    async fn handle_message_data(&self, message: SocketMessageDe) -> Result<()>;

    async fn handle_error(&self, error: Error);
}
