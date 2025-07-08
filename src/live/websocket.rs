use crate::{
    DataPoint, Error, Interval, Result, Timezone,
    chart::{ChartOptions, StudyOptions, SymbolInfo},
    live::{
        handler::{data::DataHandler, types::DataTx},
        models::{
            DataServer, Socket, SocketMessage, SocketMessageDe, SocketMessageSer,
            TradingViewDataEvent, WEBSOCKET_HEADERS,
        },
    },
    payload,
    pine_indicator::PineIndicator,
    quote::{ALL_QUOTE_FIELDS, models::QuoteValue},
    utils::{gen_id, gen_session_id, parse_packet, symbol_init},
};

use dashmap::DashMap;
use futures_util::{
    SinkExt, StreamExt,
    future::join_all,
    stream::{SplitSink, SplitStream},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fmt::Debug,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU16, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::{
    net::TcpStream,
    sync::{Mutex, MutexGuard, RwLock},
    time::timeout,
};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async_with_config,
    tungstenite::{
        client::IntoClientRequest,
        protocol::{Message, WebSocketConfig},
    },
};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, trace, warn};
use url::Url;
use ustr::{Ustr, ustr};

// Error recovery configuration
#[derive(Debug, Clone)]
struct ErrorRecoveryConfig {
    max_consecutive_errors: u64,
    error_reset_interval: Duration,
    _critical_error_threshold: Duration,
}

impl Default for ErrorRecoveryConfig {
    fn default() -> Self {
        Self {
            max_consecutive_errors: 5,
            error_reset_interval: Duration::from_secs(60),
            _critical_error_threshold: Duration::from_secs(30),
        }
    }
}

// Error statistics and tracking
#[derive(Debug, Default)]
struct ErrorStats {
    consecutive_errors: Arc<AtomicU64>,
    last_error_time: Arc<RwLock<Option<Instant>>>,
    total_errors: Arc<AtomicU64>,
    recovery_attempts: Arc<AtomicU64>,
}

impl ErrorStats {
    fn increment_error(&self) -> u64 {
        let count = self.consecutive_errors.fetch_add(1, Ordering::SeqCst) + 1;
        self.total_errors.fetch_add(1, Ordering::SeqCst);
        count
    }

    fn reset_consecutive(&self) {
        self.consecutive_errors.store(0, Ordering::SeqCst);
    }

    async fn update_last_error_time(&self) {
        let mut last_error = self.last_error_time.write().await;
        *last_error = Some(Instant::now());
    }

    async fn should_reset_consecutive_errors(&self, reset_interval: Duration) -> bool {
        let last_error = self.last_error_time.read().await;
        if let Some(last_time) = *last_error {
            last_time.elapsed() > reset_interval
        } else {
            false
        }
    }

    fn get_consecutive_errors(&self) -> u64 {
        self.consecutive_errors.load(Ordering::SeqCst)
    }
}

#[derive(Default, Clone)]
pub(crate) struct Metadata {
    pub(crate) series: Arc<DashMap<Ustr, SeriesInfo>>,
    pub(crate) studies: Arc<DashMap<Ustr, Ustr>>,
    pub(crate) quotes: Arc<DashMap<Ustr, QuoteValue>>,
    pub(crate) chart_state: Arc<RwLock<ChartState>>,
}

#[derive(Default, Clone)]
pub(crate) struct ChartState {
    pub(crate) chart: Option<(SeriesInfo, Vec<DataPoint>)>,
    pub(crate) symbol_info: Option<SymbolInfo>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SeriesInfo {
    pub chart_session: Ustr,
    pub options: ChartOptions,
}

pub struct WebSocketClient {
    pub server: DataServer,
    pub(crate) auth_token: Arc<RwLock<Ustr>>,
    pub(crate) quote_session: Arc<RwLock<Ustr>>,

    data_handler: DataHandler,
    closed: CancellationToken,
    // The following fields are used for the WebSocket connection
    is_closed: Arc<AtomicBool>,
    series_count: Arc<AtomicU16>,
    studies_count: Arc<AtomicU16>,

    read: Arc<Mutex<SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>>>,
    write: Arc<Mutex<SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>>>,

    // Error handling and recovery
    error_stats: ErrorStats,
    error_config: ErrorRecoveryConfig,
}

#[derive(Debug, Clone, Copy)]
enum ErrorSeverity {
    /// Low severity - log and continue
    Minor,
    /// Medium severity - may trigger recovery actions
    Moderate,
    /// High severity - requires immediate attention and recovery
    Critical,
    /// Fatal - connection should be terminated
    Fatal,
}
#[bon::bon]
impl WebSocketClient {
    #[builder]
    pub async fn new(
        auth_token: Option<&str>,
        #[builder(default = DataServer::ProData)] server: DataServer,
        data_tx: DataTx,
    ) -> Result<Arc<Self>> {
        let auth_token = Ustr::from(auth_token.unwrap_or("unauthorized_user_token"));

        let (write, read) = Self::connect(server).await?;

        let data_handler = DataHandler::builder().res_tx(data_tx).build();
        let is_closed = Arc::new(AtomicBool::new(false));
        let series_count = Arc::new(AtomicU16::new(0));
        let studies_count = Arc::new(AtomicU16::new(0));
        let auth_token = Arc::new(RwLock::new(auth_token));
        let write = Arc::new(Mutex::new(write));
        let read = Arc::new(Mutex::new(read));
        let quote_session = Arc::new(RwLock::new(ustr("")));

        let client = Arc::new(Self {
            data_handler,
            server,
            read,
            write,
            auth_token,
            is_closed,
            quote_session,
            series_count,
            studies_count,
            closed: CancellationToken::new(),
            error_stats: ErrorStats::default(),
            error_config: ErrorRecoveryConfig::default(),
        });

        Ok(client)
    }

    pub fn spawn_reader_task(self: Arc<Self>) {
        tokio::spawn(async move {
            if let Err(e) = self.subscribe().await {
                error!("Reader task failed: {}", e);
            }
        });
    }

    async fn connect(
        server: DataServer,
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

        // Configure WebSocket with larger message size limits
        let conf = WebSocketConfig::default()
            .read_buffer_size(1024 * 1024)
            .write_buffer_size(1024 * 1024);

        let (socket, response) = connect_async_with_config(request, Some(conf), false).await?;

        info!("WebSocket connected with status: {}", response.status());

        let (write, read) = socket.split();

        Ok((write, read))
    }

    /// Classify error severity for appropriate response
    fn classify_error_severity(&self, error: &Error, context: &str) -> ErrorSeverity {
        match error {
            // Network errors - usually recoverable
            Error::WebSocket(msg) => {
                if msg.contains("ConnectionClosed") || msg.contains("ConnectionReset") {
                    ErrorSeverity::Critical
                } else if msg.contains("timeout") {
                    ErrorSeverity::Moderate
                } else {
                    ErrorSeverity::Critical
                }
            }

            // TradingView specific errors
            Error::TradingView { source } => {
                use crate::error::TradingViewError;
                match source {
                    TradingViewError::CriticalError => ErrorSeverity::Fatal,
                    TradingViewError::ProtocolError => ErrorSeverity::Critical,
                    TradingViewError::SymbolError | TradingViewError::SeriesError => {
                        ErrorSeverity::Moderate
                    }
                    _ => ErrorSeverity::Minor,
                }
            }

            // Parse errors - usually minor unless frequent
            Error::JsonParse(_) => {
                if self.error_stats.get_consecutive_errors() > 3 {
                    ErrorSeverity::Moderate
                } else {
                    ErrorSeverity::Minor
                }
            }

            // Internal errors - context dependent
            Error::Internal(msg) => {
                if msg.contains("connection") || msg.contains("timeout") {
                    ErrorSeverity::Critical
                } else if context.contains("critical") {
                    ErrorSeverity::Fatal
                } else {
                    ErrorSeverity::Moderate
                }
            }

            // Default classification
            _ => ErrorSeverity::Moderate,
        }
    }

    /// Attempt to recover from different types of errors
    async fn attempt_error_recovery(&self, severity: ErrorSeverity, error: &Error) -> Result<bool> {
        match severity {
            ErrorSeverity::Minor => {
                // For minor errors, just log and continue
                debug!("Minor error occurred, continuing: {}", error);
                Ok(true)
            }

            ErrorSeverity::Moderate => {
                // For moderate errors, try soft recovery
                warn!(
                    "Moderate error occurred, attempting soft recovery: {}",
                    error
                );

                // Reset error count if enough time has passed
                if self
                    .error_stats
                    .should_reset_consecutive_errors(self.error_config.error_reset_interval)
                    .await
                {
                    self.error_stats.reset_consecutive();
                    info!("Reset consecutive error count after timeout period");
                }

                // Try to send a ping to test connection
                if let Err(ping_err) = self.try_ping().await {
                    warn!(
                        "Ping failed during recovery, connection may be lost: {}",
                        ping_err
                    );
                    return Ok(false);
                }

                Ok(true)
            }

            ErrorSeverity::Critical => {
                // For critical errors, attempt reconnection
                error!(
                    "Critical error occurred, attempting reconnection: {}",
                    error
                );
                self.error_stats
                    .recovery_attempts
                    .fetch_add(1, Ordering::SeqCst);

                match timeout(Duration::from_secs(30), self.reconnect()).await {
                    Ok(Ok(_)) => {
                        info!("Successfully recovered from critical error through reconnection");
                        self.error_stats.reset_consecutive();
                        Ok(true)
                    }
                    Ok(Err(reconnect_err)) => {
                        error!("Reconnection failed: {}", reconnect_err);
                        Ok(false)
                    }
                    Err(_) => {
                        error!("Reconnection timed out");
                        Ok(false)
                    }
                }
            }

            ErrorSeverity::Fatal => {
                // For fatal errors, mark connection as closed
                error!(
                    "Fatal error occurred, marking connection as closed: {}",
                    error
                );
                self.is_closed.store(true, Ordering::Relaxed);
                self.closed.cancel();
                Ok(false)
            }
        }
    }

    /// Send error information through the data handler
    async fn notify_error_handlers(&self, error: &Error, context: &str, severity: ErrorSeverity) {
        // Create context information
        let error_context = vec![serde_json::json!({
            "error_type": format!("{:?}", error),
            "context": context,
            "severity": format!("{:?}", severity),
            "consecutive_errors": self.error_stats.get_consecutive_errors(),
            "total_errors": self.error_stats.total_errors.load(Ordering::SeqCst),
            "timestamp": chrono::Utc::now().to_rfc3339(),
        })];

        // Notify through the error callback
        (self.data_handler.handler.on_error)((*error, error_context));
    }

    /// Log error with appropriate level based on severity
    fn log_error(&self, error: &Error, context: &str, severity: &ErrorSeverity) {
        let consecutive = self.error_stats.get_consecutive_errors();
        let total = self.error_stats.total_errors.load(Ordering::SeqCst);

        match severity {
            ErrorSeverity::Minor => {
                debug!(
                    "Minor error in {}: {} (consecutive: {}, total: {})",
                    context, error, consecutive, total
                );
            }
            ErrorSeverity::Moderate => {
                warn!(
                    "Moderate error in {}: {} (consecutive: {}, total: {})",
                    context, error, consecutive, total
                );
            }
            ErrorSeverity::Critical => {
                error!(
                    "Critical error in {}: {} (consecutive: {}, total: {})",
                    context, error, consecutive, total
                );
            }
            ErrorSeverity::Fatal => {
                error!(
                    "FATAL error in {}: {} (consecutive: {}, total: {})",
                    context, error, consecutive, total
                );
            }
        }
    }

    pub async fn is_closed(&self) -> bool {
        self.is_closed.load(Ordering::Relaxed)
    }

    pub async fn reconnect(&self) -> Result<()> {
        let auth_token = self.auth_token.read().await;
        let (write, read) = Self::connect(self.server).await?;
        let mut write_guard = self.write.lock().await;
        let mut read_guard = self.read.lock().await;
        *write_guard = write;
        *read_guard = read;
        self.is_closed.store(false, Ordering::Relaxed);
        self.set_auth_token(&auth_token).await?;
        Ok(())
    }

    pub async fn create_quote_session(&self) -> Result<()> {
        // Generate a new session ID for the quote session
        let session_id = gen_session_id("qs");
        self.send("quote_create_session", &payload!(session_id.clone()))
            .await?;
        let mut quote_session = self.quote_session.write().await;
        *quote_session = ustr(&session_id);
        Ok(())
    }

    pub async fn delete_quote_session(&self) -> Result<()> {
        let quote_session = self.quote_session.read().await;
        self.send("quote_delete_session", &payload!(quote_session.to_string()))
            .await?;
        Ok(())
    }

    pub async fn set_fields(&self) -> Result<()> {
        let quote_session = self.quote_session.read().await.to_string();

        let mut quote_fields = payload![quote_session];
        quote_fields.extend(ALL_QUOTE_FIELDS.clone().into_iter().map(Value::from));

        self.send("quote_set_fields", &quote_fields).await?;

        Ok(())
    }

    pub async fn add_symbols(&self, symbols: &[&str]) -> Result<()> {
        // Ensure quote session exists first
        self.ensure_quote_session().await?;

        let quote_session = self.quote_session.read().await.to_string();

        let mut payloads = payload![quote_session];
        payloads.extend(symbols.iter().map(|s| Value::from(*s)));

        self.send("quote_add_symbols", &payloads).await?;

        info!("Added {} symbols to quote session", symbols.len());
        Ok(())
    }

    pub async fn set_auth_token(&self, auth_token: &str) -> Result<()> {
        let mut auth_token_ = self.auth_token.write().await;
        *auth_token_ = Ustr::from(auth_token);
        self.send("set_auth_token", &payload!(auth_token)).await?;
        Ok(())
    }

    pub async fn send(&self, m: &str, p: &[Value]) -> Result<()> {
        if self.is_closed.load(Ordering::Relaxed) {
            return Err(Error::Internal("WebSocket is closed".into()));
        }
        debug!("Sending message: {} with payload: {:?}", m, p);
        let mut write_guard = self.write.lock().await;
        write_guard
            .send(SocketMessageSer::new(m, p).to_message()?)
            .await?;
        trace!("sent message: {} with payload: {:?}", m, p);
        drop(write_guard); // Explicitly drop the lock to avoid deadlocks
        Ok(())
    }

    pub async fn ping(&self, ping: &Message) -> Result<()> {
        let mut write_guard = self.write.lock().await;
        write_guard.send(ping.clone()).await?;
        trace!("sent ping message {}", ping);
        if ping.is_close() {
            self.is_closed.store(true, Ordering::Relaxed);
            warn!("ping message is close, closing session");
        }
        drop(write_guard); // Explicitly drop the lock to avoid deadlocks
        trace!("ping message sent successfully");
        Ok(())
    }

    pub async fn close(&self) -> Result<()> {
        self.is_closed.store(true, Ordering::Relaxed);
        self.write.lock().await.close().await?;
        Ok(())
    }

    pub async fn update_auth_token(&self, auth_token: &str) -> Result<()> {
        let mut write_guard = self.write.lock().await;
        write_guard
            .send(SocketMessageSer::new("set_auth_token", payload!(auth_token)).to_message()?)
            .await?;
        trace!("updated auth token to: {}", auth_token);
        drop(write_guard); // Explicitly drop the lock to avoid deadlocks
        Ok(())
    }

    pub async fn fast_symbols(&self, symbols: &[&str]) -> Result<()> {
        self.ensure_quote_session().await?;

        let quote_session = self.quote_session.read().await.to_string();

        let mut payloads = payload![quote_session];
        payloads.extend(symbols.iter().map(|s| Value::from(*s)));

        self.send("quote_fast_symbols", &payloads).await?;

        Ok(())
    }

    pub async fn remove_symbols(&self, symbols: &[&str]) -> Result<()> {
        let quote_session = self.quote_session.read().await.to_string();

        let mut payloads = payload![quote_session];
        payloads.extend(symbols.iter().map(|s| Value::from(*s)));

        self.send("quote_remove_symbols", &payloads).await?;

        Ok(())
    }
    // End TradingView WebSocket Quote methods

    // Begin TradingView WebSocket Chart methods
    /// Example: local = ("en", "US")
    pub async fn set_locale(&self, local: (&str, &str)) -> Result<()> {
        self.send("set_locale", &payload!(local.0, local.1)).await?;
        Ok(())
    }

    pub async fn set_data_quality(&self, data_quality: &str) -> Result<()> {
        self.send("set_data_quality", &payload!(data_quality))
            .await?;

        Ok(())
    }

    pub async fn set_timezone(&self, session: &str, timezone: Timezone) -> Result<()> {
        self.send("switch_timezone", &payload!(session, timezone.to_string()))
            .await?;

        Ok(())
    }

    pub async fn create_chart_session(&self, session: &str) -> Result<()> {
        self.send("chart_create_session", &payload!(session))
            .await?;
        Ok(())
    }

    pub async fn create_replay_session(&self, session: &str) -> Result<()> {
        self.send("replay_create_session", &payload!(session))
            .await?;
        Ok(())
    }

    pub async fn add_replay_series(
        &self,
        session: &str,
        series_id: &str,
        symbol: &str,
        config: ChartOptions,
    ) -> Result<()> {
        self.send(
            "replay_add_series",
            &payload!(
                session,
                series_id,
                symbol_init(
                    symbol,
                    config.adjustment,
                    config.currency,
                    config.session_type,
                    None
                )?,
                config.interval.to_string()
            ),
        )
        .await?;
        Ok(())
    }

    pub async fn delete_chart_session(&self, session: &str) -> Result<()> {
        self.send("chart_delete_session", &payload!(session))
            .await?;
        Ok(())
    }

    pub async fn delete_replay_session(&self, session: &str) -> Result<()> {
        self.send("replay_delete_session", &payload!(session))
            .await?;
        Ok(())
    }

    pub async fn replay_step(&self, session: &str, series_id: &str, step: u64) -> Result<()> {
        self.send("replay_step", &payload!(session, series_id, step))
            .await?;
        Ok(())
    }

    pub async fn replay_start(
        &self,
        session: &str,
        series_id: &str,
        interval: Interval,
    ) -> Result<()> {
        self.send(
            "replay_start",
            &payload!(session, series_id, interval.to_string()),
        )
        .await?;
        Ok(())
    }

    pub async fn replay_stop(&self, session: &str, series_id: &str) -> Result<()> {
        self.send("replay_stop", &payload!(session, series_id))
            .await?;
        Ok(())
    }

    pub async fn replay_reset(&self, session: &str, series_id: &str, timestamp: i64) -> Result<()> {
        self.send("replay_reset", &payload!(session, series_id, timestamp))
            .await?;
        Ok(())
    }

    pub async fn request_more_data(&self, session: &str, series_id: &str, num: u64) -> Result<()> {
        self.send("request_more_data", &payload!(session, series_id, num))
            .await?;
        Ok(())
    }

    pub async fn request_more_tickmarks(
        &self,
        session: &str,
        series_id: &str,
        num: u64,
    ) -> Result<()> {
        self.send("request_more_tickmarks", &payload!(session, series_id, num))
            .await?;
        Ok(())
    }

    pub async fn create_study(
        &self,
        session: &str,
        study_id: &str,
        series_id: &str,
        indicator: PineIndicator,
    ) -> Result<()> {
        let inputs = indicator.to_study_inputs()?;
        let payloads: Vec<Value> = vec![
            Value::from(session),
            Value::from(study_id),
            Value::from("st1"),
            Value::from(series_id),
            Value::from(indicator.script_type.to_string()),
            inputs,
        ];
        self.send("create_study", &payloads).await?;
        Ok(())
    }

    pub async fn modify_study(
        &self,
        session: &str,
        study_id: &str,
        series_id: &str,
        indicator: PineIndicator,
    ) -> Result<()> {
        let inputs = indicator.to_study_inputs()?;
        let payloads: Vec<Value> = vec![
            Value::from(session),
            Value::from(study_id),
            Value::from("st1"),
            Value::from(series_id),
            Value::from(indicator.script_type.to_string()),
            inputs,
        ];
        self.send("modify_study", &payloads).await?;
        Ok(())
    }
    pub async fn remove_study(&self, session: &str, study_id: &str) -> Result<()> {
        self.send("remove_study", &payload!(session, study_id))
            .await?;
        Ok(())
    }

    pub async fn create_series(
        &self,
        session: &str,
        series_id: &str,
        series_version: &str,
        series_symbol_id: &str,
        config: ChartOptions,
    ) -> Result<()> {
        let range = match (&config.range, config.from, config.to) {
            (Some(range), _, _) => range.to_string(),
            (None, Some(from), Some(to)) => format!("r,{from}:{to}"),
            _ => Default::default(),
        };

        self.send(
            "create_series",
            &payload!(
                session,
                series_id,
                series_version,
                series_symbol_id,
                config.interval.to_string(),
                config.bar_count,
                range // |r,1626220800:1628640000|1D|5d|1M|3M|6M|YTD|12M|60M|ALL|
            ),
        )
        .await?;

        Ok(())
    }

    pub async fn modify_series(
        &self,
        session: &str,
        series_id: &str,
        series_version: &str,
        series_symbol_id: &str,
        config: ChartOptions,
    ) -> Result<()> {
        let range = match (&config.range, config.from, config.to) {
            (Some(range), _, _) => range.to_string(),
            (None, Some(from), Some(to)) => format!("r,{from}:{to}"),
            _ => Default::default(),
        };

        self.send(
            "modify_series",
            &payload!(
                session,
                series_id,
                series_version,
                series_symbol_id,
                config.interval.to_string(),
                config.bar_count,
                range // |r,1626220800:1628640000|1D|5d|1M|3M|6M|YTD|12M|60M|ALL|
            ),
        )
        .await?;

        Ok(())
    }

    pub async fn remove_series(&self, session: &str, series_id: &str) -> Result<()> {
        self.send("remove_series", &payload!(session, series_id))
            .await?;
        Ok(())
    }

    pub async fn resolve_symbol(
        &self,
        session: &str,
        symbol_series_id: &str,
        symbol: &str,
        config: ChartOptions,
        replay_session: Option<&str>,
    ) -> Result<()> {
        self.send(
            "resolve_symbol",
            &payload!(
                session,
                symbol_series_id,
                symbol_init(
                    symbol,
                    config.adjustment,
                    config.currency,
                    config.session_type,
                    replay_session
                )?
            ),
        )
        .await?;
        Ok(())
    }

    pub async fn delete(&self) -> Result<()> {
        // Collect all sessions first to avoid holding iterator while making async calls
        let chart_sessions: Vec<Ustr> = self
            .data_handler
            .metadata
            .series
            .iter()
            .map(|entry| entry.value().chart_session)
            .collect();

        // Delete quote session first
        if let Err(e) = self.delete_quote_session().await {
            warn!("Failed to delete quote session: {:?}", e);
        }

        // Delete all chart sessions in parallel for better performance
        let delete_futures: Vec<_> = chart_sessions
            .iter()
            .map(|session| self.delete_chart_session(session))
            .collect();

        // Execute all deletions concurrently
        let results = join_all(delete_futures).await;

        // Log any errors but don't fail the entire operation
        for (i, result) in results.into_iter().enumerate() {
            if let Err(e) = result {
                warn!(
                    "Failed to delete chart session {}: {:?}",
                    chart_sessions[i], e
                );
            }
        }

        // Clear all metadata
        self.data_handler.metadata.series.clear();
        self.data_handler.metadata.studies.clear();
        self.data_handler.metadata.quotes.clear();

        // Reset counters
        self.series_count.store(0, Ordering::SeqCst);
        self.studies_count.store(0, Ordering::SeqCst);

        // Close the socket last
        if let Err(e) = self.close().await {
            error!("Failed to close socket: {:?}", e);
            return Err(e);
        }

        debug!("WebSocket client deleted successfully");
        Ok(())
    }

    // End TradingView WebSocket methods

    pub async fn set_replay(
        &self,
        symbol: &str,
        options: ChartOptions,
        chart_session: &str,
        symbol_series_id: &str,
    ) -> Result<()> {
        let replay_series_id = gen_id();
        let replay_session = gen_session_id("rs");

        self.create_replay_session(&replay_session).await?;
        self.add_replay_series(&replay_session, &replay_series_id, symbol, options)
            .await?;
        self.replay_reset(&replay_session, &replay_series_id, options.replay_from)
            .await?;
        self.resolve_symbol(
            chart_session,
            symbol_series_id,
            &options.symbol,
            options,
            Some(&replay_session),
        )
        .await?;

        Ok(())
    }

    pub async fn set_study(
        &self,
        study: StudyOptions,
        chart_session: &str,
        series_id: &str,
    ) -> Result<()> {
        let study_count = self.studies_count.fetch_add(1, Ordering::SeqCst) + 1;

        let study_id = Ustr::from(&format!("st{study_count}"));

        let indicator = PineIndicator::build()
            .fetch(&study.script_id, &study.script_version, study.script_type)
            .await?;

        let key = Ustr::from(&indicator.metadata.data.id);

        self.data_handler.metadata.studies.insert(key, study_id);

        self.create_study(chart_session, &study_id, series_id, indicator)
            .await?;
        Ok(())
    }

    pub async fn set_market(&self, options: ChartOptions) -> Result<()> {
        let series_count = self.series_count.fetch_add(1, Ordering::SeqCst) + 1;
        let symbol_series_id = format!("sds_sym_{series_count}");
        let series_id = Ustr::from(&format!("sds_{series_count}"));
        let series_version = format!("s{series_count}");
        let chart_session = Ustr::from(&gen_session_id("cs"));
        let symbol = format!("{}:{}", options.exchange, options.symbol);
        self.create_chart_session(&chart_session).await?;

        if options.replay_mode {
            self.set_replay(&symbol, options, &chart_session, &symbol_series_id)
                .await?;
        } else {
            self.resolve_symbol(&chart_session, &symbol_series_id, &symbol, options, None)
                .await?;
        }

        self.create_series(
            &chart_session,
            &series_id,
            &series_version,
            &symbol_series_id,
            options,
        )
        .await?;

        if let Some(study) = options.study_config {
            self.set_study(study, &chart_session, &series_id).await?;
        }

        let series_info = SeriesInfo {
            chart_session,
            options,
        };

        self.data_handler
            .metadata
            .series
            .insert(series_id, series_info);

        Ok(())
    }

    pub async fn subscribe(&self) -> Result<()> {
        let read = self.read.lock().await;
        if let Err(e) = self.event_loop(read).await {
            error!("Event loop failed: {}", e);
            self.is_closed.store(true, Ordering::Relaxed);
            self.closed.cancel();
            return Err(e);
        }
        Ok(())
    }

    pub async fn closed_notifier(&self) {
        self.closed.cancelled().await;
    }

    /// Fire-and-forget ping. Ignores `WouldBlock` when write buffer is full.
    pub async fn try_ping(&self) -> Result<()> {
        if self.is_closed().await {
            return Ok(());
        }
        self.ping(&Message::Ping(Vec::new().into()))
            .await
            .map_err(|e| Error::WebSocket(ustr(&format!("{e}"))))?;
        Ok(())
    }

    pub async fn ensure_quote_session(&self) -> Result<()> {
        let quote_session = self.quote_session.read().await;
        if quote_session.is_empty() {
            drop(quote_session); // Release read lock

            info!("Creating quote session");
            self.create_quote_session().await?;
            self.set_fields().await?;
        }
        Ok(())
    }
}

impl Socket for WebSocketClient {
    async fn event_loop(
        &self,
        mut read: MutexGuard<'_, SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>>,
    ) -> Result<()> {
        info!("WebSocket event loop started");

        loop {
            if self.is_closed.load(Ordering::Relaxed) {
                info!("WebSocket is closed, ending event loop");
                break;
            }

            trace!("waiting for next message");
            match timeout(Duration::from_secs(30), read.next()).await {
                Ok(Some(Ok(message))) => {
                    trace!("Received message: {:?}", message);
                    if let Err(e) = self.handle_raw_messages(message).await {
                        warn!("Error handling message: {}", e);
                        self.handle_error(e, ustr("handle_raw_messages")).await?;
                    } else {
                        // Reset consecutive errors on successful message processing
                        if self.error_stats.get_consecutive_errors() > 0 {
                            self.error_stats.reset_consecutive();
                            debug!("Reset consecutive errors after successful message processing");
                        }
                    }
                }
                Ok(Some(Err(e))) => {
                    error!("Error reading message: {:#?}", e);
                    self.is_closed.store(true, Ordering::Relaxed);
                    self.handle_error(
                        Error::WebSocket(e.to_string().into()),
                        ustr("event_loop_read"),
                    )
                    .await?;
                    return Err(Error::Internal(ustr(&e.to_string())));
                }
                Ok(None) => {
                    info!("WebSocket stream ended");
                    self.is_closed.store(true, Ordering::Relaxed);
                    break;
                }
                Err(_) => {
                    warn!("WebSocket read timeout, checking connection health");
                    if self.is_closed.load(Ordering::Relaxed) {
                        break;
                    }
                    // Send a ping to check if connection is still alive
                    if let Err(e) = self.try_ping().await {
                        warn!("Ping failed during timeout: {}", e);
                        // Don't break here, continue trying
                    }
                }
            }
        }

        info!("WebSocket event loop ended");
        Ok(())
    }

    async fn handle_raw_messages(&self, raw: Message) -> Result<()> {
        match &raw {
            Message::Text(text) => {
                debug!("Received text message: {}", text);
                self.handle_parsed_messages(parse_packet(text), &raw)
                    .await?;
            }
            Message::Close(msg) => {
                warn!("Connection closed with code: {:?}", msg);
                self.is_closed.store(true, Ordering::Relaxed);
                self.closed.cancel();
            }
            Message::Binary(msg) => {
                debug!("Received binary message: {:?}", msg);
                // TODO: handle binary messages
            }
            Message::Ping(msg) => {
                trace!("Received ping message: {:?}", msg);
                // // Send pong response
                if let Err(e) = self.ping(&Message::Pong(msg.clone())).await {
                    warn!("Failed to send pong response: {}", e);
                }
            }
            Message::Pong(msg) => {
                trace!("Received pong message: {:?}", msg);
            }
            Message::Frame(f) => {
                debug!("Received frame message: {:?}", f);
            }
        }
        Ok(())
    }

    async fn handle_parsed_messages(
        &self,
        messages: Vec<SocketMessage<SocketMessageDe>>,
        raw: &Message,
    ) -> Result<()> {
        for message in messages {
            match message {
                SocketMessage::SocketServerInfo(info) => {
                    info!("received server info: {:?}", info);
                }
                SocketMessage::SocketMessage(msg) => {
                    info!(
                        "Processing socket message: method={}, params={:?}",
                        msg.m, msg.p
                    );
                    if let Err(e) = self.handle_message_data(msg).await {
                        self.handle_error(e, ustr("handle_message_data")).await?;
                    }
                }
                SocketMessage::Other(value) => {
                    info!("Received other message: {:?}", value);
                    if value.is_number() {
                        debug!("handling heartbeat message: {:?}", value);
                        if let Err(e) = self.ping(raw).await {
                            self.handle_error(e, ustr("ping_response")).await?;
                        }
                    } else if value.is_string() {
                        info!("Received string message: {:?}", value);
                    } else {
                        warn!("unhandled message: {:?}", value);
                    }
                }
                SocketMessage::Unknown(s) => {
                    warn!("unknown message: {:?}", s);
                }
            }
        }
        Ok(())
    }

    async fn handle_message_data(&self, message: SocketMessageDe) -> Result<()> {
        debug!(
            "Handling message: method={}, params_count={}",
            message.m,
            message.p.len()
        );
        let event = TradingViewDataEvent::from(message.m.to_owned());
        debug!("Mapped to event: {:?}", event);
        self.data_handler.handle_events(event, &message.p).await;
        Ok(())
    }

    async fn handle_error(&self, error: Error, context: Ustr) -> Result<()> {
        let context_str = context.as_str();

        // Update error statistics
        let consecutive_errors = self.error_stats.increment_error();
        self.error_stats.update_last_error_time().await;

        // Classify error severity
        let severity = self.classify_error_severity(&error, context_str);

        // Log the error appropriately
        self.log_error(&error, context_str, &severity);

        // Check if we've exceeded consecutive error threshold
        if consecutive_errors >= self.error_config.max_consecutive_errors {
            error!(
                "Exceeded maximum consecutive errors ({} >= {}), marking connection as critical",
                consecutive_errors, self.error_config.max_consecutive_errors
            );

            // Escalate to critical if we have too many consecutive errors
            let escalated_severity = ErrorSeverity::Critical;

            // Notify error handlers
            self.notify_error_handlers(&error, context_str, escalated_severity)
                .await;

            // Attempt recovery
            match self
                .attempt_error_recovery(escalated_severity, &error)
                .await
            {
                Ok(recovered) => {
                    if !recovered {
                        warn!("Failed to recover from critical error state");
                        return Err(Error::Internal(ustr("Error recovery failed")));
                    }
                }
                Err(recovery_err) => {
                    error!("Error during recovery attempt: {}", recovery_err);
                    return Err(recovery_err);
                }
            }
        } else {
            // Normal error handling
            self.notify_error_handlers(&error, context_str, severity)
                .await;

            // Attempt recovery based on severity
            match self.attempt_error_recovery(severity, &error).await {
                Ok(recovered) => {
                    if !recovered {
                        warn!("Recovery attempt indicated connection should be closed");
                        return Err(Error::Internal(ustr("Connection recovery failed")));
                    }
                }
                Err(recovery_err) => {
                    error!("Error during recovery attempt: {}", recovery_err);
                    // Don't propagate recovery errors for non-fatal issues
                    match self.classify_error_severity(&error, context_str) {
                        ErrorSeverity::Fatal => return Err(recovery_err),
                        _ => {
                            warn!(
                                "Ignoring recovery error for non-fatal issue: {}",
                                recovery_err
                            );
                        }
                    }
                }
            }
        }

        Ok(())
    }
}
