use futures_util::stream::SplitStream;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::{net::TcpStream, sync::MutexGuard};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream,
    tungstenite::{
        http::{HeaderMap, HeaderValue},
        protocol::Message,
    },
};
use ustr::Ustr;

use crate::{Result, UA, error::Error, error::TradingViewError, utils::format_packet};

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
    pub sjavastudies: Vec<String>,
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

pub trait Socket {
    fn event_loop(
        &self,
        read: MutexGuard<SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>>,
    ) -> impl Future<Output = Result<()>> + Send;

    fn handle_raw_messages(
        &self,
        raw: Message,
    ) -> impl Future<Output = Result<()>> + Send;

    fn handle_parsed_messages(
        &self,
        messages: Vec<SocketMessage<SocketMessageDe>>,
        raw: &Message,
    ) -> impl Future<Output = Result<()>> + Send;

    fn handle_message_data(
        &self,
        message: SocketMessageDe,
    ) -> impl Future<Output = Result<()>> + Send;

    fn handle_error(
        &self,
        error: Error,
        context: Ustr,
    ) -> impl Future<Output = Result<()>> + Send;
}
