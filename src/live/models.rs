use core::fmt;

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

use crate::{
    Result, UA,
    error::{Error, TradingViewError},
    utils::format_packet,
};
use std::sync::LazyLock;

pub static WEBSOCKET_HEADERS: LazyLock<HeaderMap<HeaderValue>> = LazyLock::new(|| {
    let mut headers = HeaderMap::new();
    headers.insert("Origin", "https://www.tradingview.com/".parse().unwrap());
    headers.insert("User-Agent", UA.parse().unwrap());
    headers
});

#[derive(Debug, Clone, PartialEq, Eq, Hash, Copy)]
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
    UnknownEvent(Ustr),
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

            "study_loading" => TradingViewDataEvent::OnStudyLoading,
            "study_completed" => TradingViewDataEvent::OnStudyCompleted,

            "symbol_error" => TradingViewDataEvent::OnError(TradingViewError::SymbolError),
            "series_error" => TradingViewDataEvent::OnError(TradingViewError::SeriesError),
            "critical_error" => TradingViewDataEvent::OnError(TradingViewError::CriticalError),
            "study_error" => TradingViewDataEvent::OnError(TradingViewError::StudyError),
            "protocol_error" => TradingViewDataEvent::OnError(TradingViewError::ProtocolError),
            "replay_error" => TradingViewDataEvent::OnError(TradingViewError::ReplayError),

            s => TradingViewDataEvent::UnknownEvent(s.into()),
        }
    }
}

impl From<Ustr> for TradingViewDataEvent {
    fn from(s: Ustr) -> Self {
        TradingViewDataEvent::from(s.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SocketMessageSer {
    pub m: Value,
    pub p: Value,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct SocketMessageDe {
    pub m: Ustr,
    pub p: Vec<Value>,
    pub t: u64,    // Timestamp in seconds
    pub t_ms: u64, // Timestamp in milliseconds
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
    pub session_id: Ustr,
    pub timestamp: i64,
    pub timestamp_ms: i64,
    pub release: Ustr,
    #[serde(rename = "studies_metadata_hash")]
    pub studies_metadata_hash: Ustr,
    #[serde(rename = "auth_scheme_vsn")]
    pub auth_scheme_vsn: i64,
    pub protocol: Ustr,
    pub via: Ustr,
    pub sjavastudies: Vec<Ustr>,
}

impl fmt::Display for SocketServerInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "SocketServerInfo {{ session_id: {}, timestamp: {}, timestamp_ms: {}, release: {}, studies_metadata_hash: {}, auth_scheme_vsn: {}, protocol: {}, via: {}, sjavastudies: {:?} }}",
            self.session_id,
            self.timestamp,
            self.timestamp_ms,
            self.release,
            self.studies_metadata_hash,
            self.auth_scheme_vsn,
            self.protocol,
            self.via,
            self.sjavastudies
        )
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum SocketMessage<T> {
    SocketServerInfo(SocketServerInfo),
    SocketMessage(T),
    Other(Value),
    Unknown(Ustr),
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

    fn handle_raw_messages(&self, raw: Message) -> impl Future<Output = Result<()>> + Send;

    fn handle_parsed_messages(
        &self,
        messages: Vec<SocketMessage<SocketMessageDe>>,
        raw: &Message,
    ) -> impl Future<Output = Result<()>> + Send;

    fn handle_message_data(
        &self,
        message: SocketMessageDe,
    ) -> impl Future<Output = Result<()>> + Send;

    fn handle_error(&self, error: Error, context: Ustr) -> impl Future<Output = Result<()>> + Send;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_study_loading_maps_to_on_study_loading() {
        let event = TradingViewDataEvent::from("study_loading".to_string());
        assert_eq!(event, TradingViewDataEvent::OnStudyLoading);
    }

    #[test]
    fn test_series_loading_is_not_study_loading() {
        let event = TradingViewDataEvent::from("series_loading".to_string());
        assert_eq!(event, TradingViewDataEvent::OnSeriesLoading);
    }

    #[test]
    fn test_study_completed_maps_correctly() {
        let event = TradingViewDataEvent::from("study_completed".to_string());
        assert_eq!(event, TradingViewDataEvent::OnStudyCompleted);
    }

    #[test]
    fn test_series_completed_maps_correctly() {
        let event = TradingViewDataEvent::from("series_completed".to_string());
        assert_eq!(event, TradingViewDataEvent::OnSeriesCompleted);
    }

    #[test]
    fn test_all_study_and_series_events_are_distinct() {
        let loading = TradingViewDataEvent::from("study_loading".to_string());
        let completed = TradingViewDataEvent::from("study_completed".to_string());
        let series_loading = TradingViewDataEvent::from("series_loading".to_string());
        let series_completed = TradingViewDataEvent::from("series_completed".to_string());

        // All four should be distinct
        assert_ne!(loading, series_loading);
        assert_ne!(completed, series_completed);
        assert_ne!(loading, completed);
        assert_ne!(series_loading, series_completed);

        // Verify mapping expectations
        assert_eq!(loading, TradingViewDataEvent::OnStudyLoading);
        assert_eq!(completed, TradingViewDataEvent::OnStudyCompleted);
        assert_eq!(series_loading, TradingViewDataEvent::OnSeriesLoading);
        assert_eq!(series_completed, TradingViewDataEvent::OnSeriesCompleted);
    }

    #[test]
    fn test_ustr_from_maps_correctly() {
        let event: TradingViewDataEvent = ustr::ustr("study_loading").into();
        assert_eq!(event, TradingViewDataEvent::OnStudyLoading);
    }
}
