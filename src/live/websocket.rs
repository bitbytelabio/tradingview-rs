use futures_util::{
    SinkExt, StreamExt,
    stream::{SplitSink, SplitStream},
};
use iso_currency::Currency;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    fmt::Debug,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tokio::{
    net::TcpStream,
    sync::{Mutex, MutexGuard, RwLock, mpsc},
    task::JoinHandle,
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

use crate::{
    Error, Interval, MarketAdjustment, Result, SessionType, SocketServerInfo, Timezone,
    chart::{ChartOptions, options::Range},
    live::{
        handler::Handler,
        models::{
            DataServer, Socket, SocketMessage, SocketMessageDe, SocketMessageSer,
            TradingViewDataEvent, WEBSOCKET_HEADERS,
        },
    },
    payload,
    quote::ALL_QUOTE_FIELDS,
    study::StudyConfiguration,
    utils::{parse_packet, symbol_init},
};

// Error recovery configuration
#[derive(Debug, Clone, Copy)]
pub(crate) struct ErrorRecoveryConfig {
    max_consecutive_errors: u64,
    error_reset_interval: Duration,
    max_recovery_attempts: u32,
    backoff_base_delay: Duration,
    backoff_max_delay: Duration,
    connection_timeout: Duration,
    ping_interval: Duration,
    health_check_interval: Duration,
}

impl Default for ErrorRecoveryConfig {
    fn default() -> Self {
        Self {
            max_consecutive_errors: 5,
            error_reset_interval: Duration::from_secs(60),
            max_recovery_attempts: 3,
            backoff_base_delay: Duration::from_millis(100),
            backoff_max_delay: Duration::from_secs(30),
            connection_timeout: Duration::from_secs(30),
            ping_interval: Duration::from_secs(30),
            health_check_interval: Duration::from_secs(60),
        }
    }
}

#[derive(Debug, Clone)]
struct ErrorStats {
    consecutive_errors: Arc<AtomicU64>,
    last_error_time: Arc<RwLock<Option<Instant>>>,
    total_errors: Arc<AtomicU64>,
    recovery_attempts: Arc<AtomicU64>,
    connection_drops: Arc<AtomicU64>,
    last_successful_message: Arc<RwLock<Option<Instant>>>,
    recent_critical_times: Arc<RwLock<[u64; 4]>>,
    recent_critical_count: Arc<AtomicU64>,
}

impl Default for ErrorStats {
    fn default() -> Self {
        Self {
            consecutive_errors: Arc::new(AtomicU64::new(0)),
            last_error_time: Arc::new(RwLock::new(None)),
            total_errors: Arc::new(AtomicU64::new(0)),
            recovery_attempts: Arc::new(AtomicU64::new(0)),
            connection_drops: Arc::new(AtomicU64::new(0)),
            last_successful_message: Arc::new(RwLock::new(Some(Instant::now()))),
            recent_critical_times: Arc::new(RwLock::new([0u64; 4])),
            recent_critical_count: Arc::new(AtomicU64::new(0)),
        }
    }
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

    async fn update_last_successful_message(&self) {
        let mut last_success = self.last_successful_message.write().await;
        *last_success = Some(Instant::now());
    }

    async fn record_critical_error(&self, now_secs: u64) {
        let mut times = self.recent_critical_times.write().await;
        times[0] = times[1];
        times[1] = times[2];
        times[2] = times[3];
        times[3] = now_secs;
        self.recent_critical_count.fetch_add(1, Ordering::Relaxed);
    }

    async fn should_reset_consecutive_errors(&self, reset_interval: Duration) -> bool {
        let last_error = self.last_error_time.read().await;
        if let Some(last_time) = *last_error {
            last_time.elapsed() > reset_interval
        } else {
            false
        }
    }

    async fn is_connection_stale(&self, stale_threshold: Duration) -> bool {
        let last_success = self.last_successful_message.read().await;
        if let Some(last_time) = *last_success {
            last_time.elapsed() > stale_threshold
        } else {
            true // No successful messages yet
        }
    }

    fn get_consecutive_errors(&self) -> u64 {
        self.consecutive_errors.load(Ordering::SeqCst)
    }

    async fn get_recent_critical_count(&self, window_secs: u64) -> usize {
        if self.recent_critical_count.load(Ordering::Relaxed) == 0 {
            return 0;
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        if let Ok(times) = self.recent_critical_times.try_read() {
            times
                .iter()
                .filter(|&&t| t > 0 && now.saturating_sub(t) <= window_secs)
                .count()
        } else {
            0
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
/// Severity classification for WebSocket errors.
///
/// Used by the error recovery system to decide whether to retry, reconnect,
/// or open the circuit breaker.
pub enum ErrorSeverity {
    /// Trace level - very minor, log only
    Trace,
    /// Low severity - log and continue
    Minor,
    /// Medium severity - may trigger recovery actions
    Moderate,
    /// High severity - requires immediate attention and recovery
    Critical,
    /// Fatal - connection should be terminated
    Fatal,
}

// Connection health tracking
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
/// Health status of a WebSocket connection.
pub enum ConnectionHealth {
    Healthy,
    Degraded,
    Unstable,
    Failed,
}

#[derive(Debug, Clone, Copy)]
struct HealthMetrics {
    health: ConnectionHealth,
    last_ping_time: Option<Instant>,
    avg_response_time: Duration,
}

impl Default for HealthMetrics {
    fn default() -> Self {
        Self {
            health: ConnectionHealth::Healthy,
            last_ping_time: None,
            avg_response_time: Duration::from_millis(0),
        }
    }
}

/// Metadata for a chart data series returned by TradingView.
///
/// Includes the chart session ID and the [`ChartOptions`] used to request it.
///
/// [`ChartOptions`]: crate::chart::ChartOptions
#[derive(Debug, Clone, Default, Deserialize, Serialize, Copy)]
pub struct SeriesInfo {
    pub chart_session: Ustr,
    pub options: ChartOptions,
}

/// The primary WebSocket client for TradingView real-time data.
///
/// Connects to a TradingView data server, authenticates with the provided token,
/// and streams chart data, quotes, and study results via the handler trait.
///
/// # Architecture
///
/// The client uses a channel-based write path (no lock contention on sends)
/// and a dedicated reader task that dispatches parsed messages to the handler.
///
/// # Example
///
/// ```rust,ignore
/// let ws = WebSocketClient::builder()
///     .auth_token("your_token")
///     .server(DataServer::ProData)
///     .handler(my_handler)
///     .build()
///     .await?;
/// ```
pub struct WebSocketClient<T: Handler> {
    pub server: DataServer,
    pub auth_token: Arc<RwLock<Ustr>>,

    handler: T,

    // WebSocket connection
    cancellation: CancellationToken,
    is_closed: Arc<AtomicBool>,

    read: Arc<Mutex<SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>>>,
    /// Channel-based write path — eliminates `Arc<Mutex<SplitSink>>` lock contention.
    /// All outgoing messages are sent on this channel; a dedicated writer task
    /// drains it and writes to the `SplitSink`.
    ///
    /// Wrapped in `Arc<RwLock<>>`: the hot `send()` path acquires a read lock
    /// (concurrent, cheap).  Only `reconnect()` acquires the write lock.
    write_tx: Arc<RwLock<mpsc::Sender<Message>>>,
    /// Handle to the writer task so we can abort it on reconnect.
    writer_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
    buffer_size: usize,

    // Enhanced error handling and recovery
    error_stats: ErrorStats,
    error_config: ErrorRecoveryConfig,
    health_metrics: Arc<RwLock<HealthMetrics>>,

    // Circuit breaker state
    circuit_breaker_open: Arc<AtomicBool>,
    circuit_breaker_opened_at: Arc<RwLock<Option<Instant>>>,
}

#[bon::bon]
#[allow(private_interfaces)]
impl<T: Handler> WebSocketClient<T> {
    #[builder]
    pub async fn new(
        auth_token: Option<&str>,
        #[builder(default = DataServer::ProData)] server: DataServer,
        handler: T,
        #[builder(default = 1024*1024)] buffer_size: usize,
        #[builder(default)] error_config: ErrorRecoveryConfig,
    ) -> Result<Arc<Self>> {
        let auth_token = Ustr::from(auth_token.unwrap_or("unauthorized_user_token"));
        let (write, read) = Self::connect(server, Some(buffer_size)).await?;

        let is_closed = Arc::new(AtomicBool::new(false));
        let auth_token = Arc::new(RwLock::new(auth_token));
        let read = Arc::new(Mutex::new(read));

        // Channel-based write path: 1024 messages of buffering before backpressure.
        let (write_tx, write_rx) = mpsc::channel(1024);
        let write_tx = Arc::new(RwLock::new(write_tx));
        let writer_handle = Arc::new(Mutex::new(None::<JoinHandle<()>>));

        let client = Arc::new(Self {
            handler,
            server,
            read,
            write_tx: write_tx.clone(),
            writer_handle: writer_handle.clone(),
            auth_token,
            is_closed: is_closed.clone(),
            buffer_size,
            cancellation: CancellationToken::new(),
            error_stats: ErrorStats::default(),
            error_config,
            health_metrics: Arc::new(RwLock::new(HealthMetrics::default())),
            circuit_breaker_open: Arc::new(AtomicBool::new(false)),
            circuit_breaker_opened_at: Arc::new(RwLock::new(None)),
        });

        // Spawn the dedicated writer task.
        Self::spawn_writer(write, write_rx, writer_handle, is_closed.clone());

        // Start health monitoring task
        client.spawn_health_monitor();

        Ok(client)
    }

    pub fn spawn_reader_task(self: Arc<Self>) {
        tokio::spawn(async move {
            if let Err(e) = self.subscribe().await {
                error!("Reader task failed: {}", e);
            }
        });
    }

    /// Spawn a dedicated writer task that owns the `SplitSink` directly (no
    /// `Mutex`) and drains the mpsc channel.  Eliminates write lock contention.
    fn spawn_writer(
        mut sink: SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>,
        mut rx: mpsc::Receiver<Message>,
        handle_storage: Arc<Mutex<Option<JoinHandle<()>>>>,
        is_closed: Arc<AtomicBool>,
    ) {
        let handle = tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if is_closed.load(Ordering::Relaxed) {
                    break;
                }
                if sink.send(msg).await.is_err() {
                    is_closed.store(true, Ordering::Relaxed);
                    break;
                }
            }
            // Channel closed or connection dead — close the sink.
            let _ = sink.close().await;
            is_closed.store(true, Ordering::Relaxed);
        });

        // Store the handle so reconnect can abort it.
        tokio::spawn(async move {
            let mut guard = handle_storage.lock().await;
            *guard = Some(handle);
        });
    }

    async fn connect(
        server: DataServer,
        buffer_size: Option<usize>,
    ) -> Result<(
        SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>,
        SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    )> {
        let url = Url::parse(&format!(
            "wss://{server}.tradingview.com/socket.io/websocket"
        ))?;

        let buffer_size = buffer_size.unwrap_or(1024 * 1024);

        let mut request = url.into_client_request()?;
        request.headers_mut().extend((*WEBSOCKET_HEADERS).clone());

        // Configure WebSocket with larger message size limits
        let conf = WebSocketConfig::default()
            .read_buffer_size(buffer_size)
            .write_buffer_size(buffer_size);

        let (socket, response) = connect_async_with_config(request, Some(conf), false).await?;

        info!("WebSocket connected with status: {}", response.status());

        let (write, read) = socket.split();

        Ok((write, read))
    }

    /// Classify error severity for appropriate response
    async fn classify_error_severity(&self, error: &Error, context: &str) -> ErrorSeverity {
        // Check recent error pattern for escalation
        let consecutive_errors = self.error_stats.get_consecutive_errors();
        let recent_critical = self.error_stats.get_recent_critical_count(300).await;

        // Pattern-based escalation
        let base_severity = match error {
            Error::WebSocket(msg) => {
                if msg.contains("ConnectionClosed") || msg.contains("ConnectionReset") {
                    ErrorSeverity::Critical
                } else if msg.contains("timeout") || msg.contains("WouldBlock") {
                    ErrorSeverity::Moderate
                } else if msg.contains("Protocol") {
                    ErrorSeverity::Critical
                } else {
                    ErrorSeverity::Moderate
                }
            }

            Error::TradingView { source } => {
                use crate::error::TradingViewError;
                match source {
                    TradingViewError::CriticalError => ErrorSeverity::Fatal,
                    TradingViewError::ProtocolError => ErrorSeverity::Critical,
                    TradingViewError::SymbolError | TradingViewError::SeriesError => {
                        ErrorSeverity::Minor
                    }
                    _ => ErrorSeverity::Trace,
                }
            }

            Error::JsonParse(_) => {
                if consecutive_errors > 3 {
                    ErrorSeverity::Moderate
                } else {
                    ErrorSeverity::Minor
                }
            }

            Error::Internal(msg) => {
                if msg.contains("connection") || msg.contains("timeout") {
                    ErrorSeverity::Critical
                } else if context.contains("critical") {
                    ErrorSeverity::Fatal
                } else {
                    ErrorSeverity::Moderate
                }
            }

            _ => ErrorSeverity::Moderate,
        };

        // Escalate based on error patterns
        if recent_critical >= 3 {
            ErrorSeverity::Fatal
        } else if consecutive_errors >= self.error_config.max_consecutive_errors {
            std::cmp::max(base_severity, ErrorSeverity::Critical)
        } else {
            base_severity
        }
    }

    async fn attempt_error_recovery(&self, severity: ErrorSeverity, error: &Error) -> Result<bool> {
        // Check circuit breaker
        if self.is_circuit_breaker_open().await {
            warn!("Circuit breaker is open, skipping recovery attempt");
            return Ok(false);
        }

        match severity {
            ErrorSeverity::Trace => {
                trace!("Trace level error, no action needed: {}", error);
                Ok(true)
            }

            ErrorSeverity::Minor => {
                debug!("Minor error occurred, continuing: {}", error);
                Ok(true)
            }

            ErrorSeverity::Moderate => {
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

                // Health check
                if let Err(health_err) = self.perform_health_check().await {
                    warn!("Health check failed during recovery: {}", health_err);
                    return Ok(false);
                }

                Ok(true)
            }

            ErrorSeverity::Critical => {
                error!(
                    "Critical error occurred, attempting reconnection: {}",
                    error
                );
                self.error_stats
                    .recovery_attempts
                    .fetch_add(1, Ordering::SeqCst);

                // Try recovery with exponential backoff
                for attempt in 1..=self.error_config.max_recovery_attempts {
                    let delay = self.calculate_backoff_delay(attempt);
                    warn!("Recovery attempt {} after {:?} delay", attempt, delay);

                    tokio::time::sleep(delay).await;

                    match timeout(self.error_config.connection_timeout, self.reconnect()).await {
                        Ok(Ok(_)) => {
                            info!(
                                "Successfully recovered from critical error (attempt {})",
                                attempt
                            );
                            self.error_stats.reset_consecutive();
                            return Ok(true);
                        }
                        Ok(Err(reconnect_err)) => {
                            error!("Reconnection attempt {} failed: {}", attempt, reconnect_err);
                        }
                        Err(_) => {
                            error!("Reconnection attempt {} timed out", attempt);
                        }
                    }
                }

                // All recovery attempts failed, open circuit breaker
                self.open_circuit_breaker().await;
                Ok(false)
            }

            ErrorSeverity::Fatal => {
                error!("Fatal error occurred, terminating connection: {}", error);
                self.is_closed.store(true, Ordering::Relaxed);
                self.cancellation.cancel();
                self.open_circuit_breaker().await;
                Ok(false)
            }
        }
    }

    /// Calculate exponential backoff delay
    fn calculate_backoff_delay(&self, attempt: u32) -> Duration {
        let delay = self
            .error_config
            .backoff_base_delay
            .mul_f64((2_f64).powi(attempt as i32 - 1));

        std::cmp::min(delay, self.error_config.backoff_max_delay)
    }

    /// Circuit breaker management
    async fn is_circuit_breaker_open(&self) -> bool {
        if !self.circuit_breaker_open.load(Ordering::Relaxed) {
            return false;
        }

        // Check if circuit breaker should be reset
        let opened_at = self.circuit_breaker_opened_at.read().await;
        if let Some(time) = *opened_at {
            if time.elapsed() > Duration::from_secs(300) {
                // 5 minutes
                self.circuit_breaker_open.store(false, Ordering::Relaxed);
                info!("Circuit breaker reset after timeout");
                return false;
            }
        }

        true
    }

    async fn open_circuit_breaker(&self) {
        self.circuit_breaker_open.store(true, Ordering::Relaxed);
        let mut opened_at = self.circuit_breaker_opened_at.write().await;
        *opened_at = Some(Instant::now());
        error!("Circuit breaker opened due to repeated failures");
    }

    /// health check
    async fn perform_health_check(&self) -> Result<()> {
        if self.is_closed() {
            return Err(Error::Internal(ustr("Connection is closed")));
        }

        // Check if connection is stale
        if self
            .error_stats
            .is_connection_stale(Duration::from_secs(120))
            .await
        {
            warn!("Connection appears stale, performing ping test");
        }

        // Send ping and measure response
        let start = Instant::now();
        self.try_ping().await?;
        let ping_duration = start.elapsed();

        // Update health metrics
        let mut metrics = self.health_metrics.write().await;
        metrics.last_ping_time = Some(start);

        // Update average response time (simple moving average)
        if metrics.avg_response_time.is_zero() {
            metrics.avg_response_time = ping_duration;
        } else {
            metrics.avg_response_time = Duration::from_nanos(
                (metrics.avg_response_time.as_nanos() as f64 * 0.8
                    + ping_duration.as_nanos() as f64 * 0.2) as u64,
            );
        }

        // Determine health status
        metrics.health = if ping_duration > Duration::from_secs(5) {
            ConnectionHealth::Degraded
        } else if self.error_stats.get_consecutive_errors() > 2 {
            ConnectionHealth::Unstable
        } else {
            ConnectionHealth::Healthy
        };

        debug!(
            "Health check completed: {:?}, ping: {:?}",
            metrics.health, ping_duration
        );
        Ok(())
    }

    /// Spawn background health monitoring task
    fn spawn_health_monitor(self: &Arc<Self>) {
        let client = Arc::clone(self);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(client.error_config.health_check_interval);

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if client.is_closed() {
                            break;
                        }

                        if let Err(e) = client.perform_health_check().await {
                            warn!("Scheduled health check failed: {}", e);
                        }
                    }
                    _ = client.cancellation.cancelled() => {
                        debug!("Health monitor task cancelled");
                        break;
                    }
                }
            }
        });
    }

    /// Enhanced error notification with context
    async fn notify_error_handlers(&self, error: &Error, context: &str, severity: ErrorSeverity) {
        // Record error in history
        if severity >= ErrorSeverity::Critical {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            self.error_stats.record_critical_error(now).await;
        }

        // Create comprehensive context information
        let health_metrics = self.health_metrics.read().await;
        let error_context = vec![json!({
            "error_type": format!("{:?}", error),
            "context": context,
            "severity": format!("{:?}", severity),
            "consecutive_errors": self.error_stats.get_consecutive_errors(),
            "total_errors": self.error_stats.total_errors.load(Ordering::SeqCst),
            "recovery_attempts": self.error_stats.recovery_attempts.load(Ordering::SeqCst),
            "connection_health": format!("{:?}", health_metrics.health),
            "avg_response_time_ms": health_metrics.avg_response_time.as_millis(),
            "circuit_breaker_open": self.circuit_breaker_open.load(Ordering::Relaxed),
            "timestamp": chrono::Utc::now().to_rfc3339(),
        })];

        // Notify through the error callback
        self.handler.notify_error(*error, &error_context);
    }

    /// Enhanced error logging with structured information
    fn log_error(&self, error: &Error, context: &str, severity: &ErrorSeverity) {
        let consecutive = self.error_stats.get_consecutive_errors();
        let total = self.error_stats.total_errors.load(Ordering::SeqCst);
        let recovery_attempts = self.error_stats.recovery_attempts.load(Ordering::SeqCst);

        let error_info = format!(
            "{} (consecutive: {}, total: {}, recovery_attempts: {})",
            error, consecutive, total, recovery_attempts
        );

        match severity {
            ErrorSeverity::Trace => {
                trace!("Trace error in {}: {}", context, error_info);
            }
            ErrorSeverity::Minor => {
                debug!("Minor error in {}: {}", context, error_info);
            }
            ErrorSeverity::Moderate => {
                warn!("Moderate error in {}: {}", context, error_info);
            }
            ErrorSeverity::Critical => {
                error!("Critical error in {}: {}", context, error_info);
            }
            ErrorSeverity::Fatal => {
                error!("FATAL error in {}: {}", context, error_info);
            }
        }
    }

    pub fn is_closed(&self) -> bool {
        self.is_closed.load(Ordering::Relaxed)
    }

    pub async fn reconnect(&self) -> Result<()> {
        let auth_token = self.auth_token.read().await;

        // Abort the old writer task.
        let mut wh = self.writer_handle.lock().await;
        if let Some(handle) = wh.take() {
            handle.abort();
        }
        drop(wh);

        let (write, read) = Self::connect(self.server, Some(self.buffer_size)).await?;

        // Create a new channel and spawn a new writer task.
        let (new_tx, new_rx) = mpsc::channel(1024);
        Self::spawn_writer(
            write,
            new_rx,
            self.writer_handle.clone(),
            self.is_closed.clone(),
        );

        // Atomically swap the sender so future writes go to the new connection.
        {
            let mut tx_guard = self.write_tx.write().await;
            *tx_guard = new_tx;
        }

        let mut read_guard = self.read.lock().await;
        *read_guard = read;
        self.is_closed.store(false, Ordering::Relaxed);
        self.set_auth_token(&auth_token).await?;
        Ok(())
    }

    #[tracing::instrument(skip(self), level = "debug")]
    pub async fn send_raw_message(&self, message: &str) -> Result<()> {
        if self.is_closed.load(Ordering::Relaxed) {
            return Err(Error::Internal("WebSocket is closed".into()));
        }

        // Check circuit breaker
        if self.is_circuit_breaker_open().await {
            return Err(Error::Internal("Circuit breaker is open".into()));
        }

        let tx = self.write_tx.read().await;
        match timeout(
            Duration::from_secs(10),
            tx.send(Message::Text(message.into())),
        )
        .await
        {
            Ok(Ok(_)) => {
                self.error_stats.update_last_successful_message().await;
                Ok(())
            }
            Ok(Err(e)) => Err(Error::WebSocket(e.to_string().into())),
            Err(_) => Err(Error::Internal("Send timeout".into())),
        }
    }

    #[tracing::instrument(skip(self), level = "debug")]
    pub async fn send(&self, m: &str, p: &[Value]) -> Result<()> {
        if self.is_closed.load(Ordering::Relaxed) {
            return Err(Error::Internal("WebSocket is closed".into()));
        }
        let tx = self.write_tx.read().await;
        tx.send(SocketMessageSer::new(m, p).to_message()?)
            .await
            .map_err(|e| Error::WebSocket(e.to_string().into()))?;
        Ok(())
    }

    #[tracing::instrument(skip(self), level = "debug")]
    pub async fn ping(&self, ping: &Message) -> Result<()> {
        let tx = self.write_tx.read().await;
        tx.send(ping.clone())
            .await
            .map_err(|e| Error::WebSocket(e.to_string().into()))?;
        if ping.is_close() {
            self.is_closed.store(true, Ordering::Relaxed);
            tracing::warn!("ping message is close, closing session");
        }
        Ok(())
    }

    pub async fn close(&self) -> Result<()> {
        self.is_closed.store(true, Ordering::Relaxed);
        // Drop the send half of the channel — this signals the writer task
        // that no more messages are coming, causing `rx.recv()` to return
        // `None` and the writer task to exit.
        let (dummy_tx, _dummy_rx) = mpsc::channel::<Message>(1);
        let mut tx_guard = self.write_tx.write().await;
        let old_tx = std::mem::replace(&mut *tx_guard, dummy_tx);
        drop(old_tx); // closes the old channel
        drop(tx_guard);
        Ok(())
    }

    #[tracing::instrument(skip(self), level = "debug")]
    pub async fn fast_symbols(&self, quote_session: &str, symbols: &[&str]) -> Result<()> {
        let mut payloads = payload![quote_session];
        payloads.extend(symbols.iter().map(|s| Value::from(*s)));
        self.send("quote_fast_symbols", &payloads).await?;
        Ok(())
    }

    #[tracing::instrument(skip(self), level = "debug")]
    pub async fn create_quote_session(&self, quote_session: &str) -> Result<()> {
        self.send("quote_create_session", &payload!(quote_session))
            .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self), level = "debug")]
    pub async fn delete_quote_session(&self, quote_session: &str) -> Result<()> {
        self.send("quote_delete_session", &payload!(quote_session))
            .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self), level = "debug")]
    pub async fn set_fields(&self, quote_session: &str) -> Result<()> {
        let mut quote_fields = payload![quote_session];
        quote_fields.extend(ALL_QUOTE_FIELDS.iter().copied().map(|s| Value::from(s)));
        self.send("quote_set_fields", &quote_fields).await?;
        Ok(())
    }

    #[tracing::instrument(skip(self), level = "debug")]
    pub async fn add_symbols(&self, quote_session: &str, symbols: &[&str]) -> Result<()> {
        let mut payloads = payload![quote_session];
        payloads.extend(symbols.iter().map(|s| Value::from(*s)));
        self.send("quote_add_symbols", &payloads).await?;
        info!("Added {} symbols to quote session", symbols.len());
        Ok(())
    }

    #[tracing::instrument(skip(self), level = "debug")]
    pub async fn remove_symbols(&self, quote_session: &str, symbols: &[&str]) -> Result<()> {
        let mut payloads = payload![quote_session];
        payloads.extend(symbols.iter().map(|s| Value::from(*s)));
        self.send("quote_remove_symbols", &payloads).await?;
        Ok(())
    }

    #[tracing::instrument(skip(self), level = "debug")]
    pub async fn set_auth_token(&self, auth_token: &str) -> Result<()> {
        let mut auth_token_ = self.auth_token.write().await;
        *auth_token_ = ustr(auth_token);
        self.send("set_auth_token", &payload!(auth_token)).await?;
        Ok(())
    }

    /// Example: locale = ("en", "US")
    #[tracing::instrument(skip(self), level = "debug")]
    pub async fn set_locale(&self, language_code: &str, country: &str) -> Result<()> {
        self.send("set_locale", &payload!(language_code, country))
            .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self), level = "debug")]
    pub async fn set_data_quality(&self, data_quality: &str) -> Result<()> {
        self.send("set_data_quality", &payload!(data_quality))
            .await?;

        Ok(())
    }

    #[tracing::instrument(skip(self), level = "debug")]
    pub async fn set_timezone(&self, chart_session: &str, timezone: Timezone) -> Result<()> {
        self.send(
            "switch_timezone",
            &payload!(chart_session, timezone.to_string()),
        )
        .await?;

        Ok(())
    }

    #[tracing::instrument(skip(self), level = "debug")]
    pub async fn create_chart_session(&self, session: &str) -> Result<()> {
        // Protocol spec: chart_create_session takes 2 args: [session_id, ""]
        // The 2nd arg is an empty string, consistent with protocol spec.
        self.send("chart_create_session", &payload!(session, ""))
            .await?;
        Ok(())
    }

    /// Create a chart data series.
    ///
    /// # Protocol (corrected)
    ///
    /// TradingView uses **two mutually exclusive modes**:
    ///
    /// **Count mode** — 6 args, for live streaming and N-bar lookback:
    ///   `["cs_xxx", "sds_1", "s1", "sds_sym_1", "1D", 300]`
    ///
    /// **Range mode** — 7 args, for historical date-range fetch:
    ///   `["cs_xxx", "sds_1", "s1", "sds_sym_1", "1D", 0, "r,from_unix:to_unix"]`
    ///
    /// **FIX**: The old code always sent 7 args, passing an empty string
    /// `""` for the 7th in count mode. The server interprets 7 args as
    /// range mode, fails on the empty range, and emits
    /// `critical_error: "unsupported method: du"`.
    #[tracing::instrument(skip(self), level = "debug")]
    #[builder]
    pub async fn create_series(
        &self,
        chart_session: &str,
        series_identifier: &str, // (sds_2)
        series_id: &str,         // (s1)
        symbol_series_id: &str,  // (sds_sym_2)
        interval: Interval,
        bar_count: u64,
        range: Option<Range>,
    ) -> Result<()> {
        // Count mode: 6 args (no range).
        // Range mode: 7 args with `bar_count = 0` and `range = "r,from:to"`.
        if let Some(r) = range {
            let args = payload!(
                chart_session,
                series_identifier,
                series_id,
                symbol_series_id,
                interval.to_string(),
                0u64,          // bar_count MUST be 0 in range mode
                r.to_string()  // "r,1626220800:1628640000"
            );
            self.send("create_series", &args).await?;
        } else {
            let args: Vec<Value> = vec![
                Value::from(chart_session),
                Value::from(series_identifier),
                Value::from(series_id),
                Value::from(symbol_series_id),
                Value::from(interval.to_string()),
                Value::from(bar_count),
            ];
            self.send("create_series", &args).await?;
        }

        Ok(())
    }

    /// Modify an existing chart series (e.g., change timeframe).
    ///
    /// Same count/range mode distinction as [`create_series`].
    #[tracing::instrument(skip(self), level = "debug")]
    #[builder]
    pub async fn modify_series(
        &self,
        chart_session: &str,
        series_identifier: &str, // (sds_2)
        series_id: &str,         // (s1)
        symbol_series_id: &str,  // (sds_sym_2)
        interval: Interval,
        bar_count: u64,
        range: Option<Range>,
    ) -> Result<()> {
        if let Some(r) = range {
            let args = payload!(
                chart_session,
                series_identifier,
                series_id,
                symbol_series_id,
                interval.to_string(),
                0u64,
                r.to_string()
            );
            self.send("modify_series", &args).await?;
        } else {
            let args: Vec<Value> = vec![
                Value::from(chart_session),
                Value::from(series_identifier),
                Value::from(series_id),
                Value::from(symbol_series_id),
                Value::from(interval.to_string()),
                Value::from(bar_count),
            ];
            self.send("modify_series", &args).await?;
        }

        Ok(())
    }

    #[tracing::instrument(skip(self), level = "debug")]
    pub async fn remove_series(&self, chart_session: &str, series_identifier: &str) -> Result<()> {
        self.send("remove_series", &payload!(chart_session, series_identifier))
            .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self), level = "debug")]
    #[builder]
    pub async fn resolve_symbol(
        &self,
        session: &str,
        symbol_series_id: &str,
        instrument: &str,
        adjustment: Option<MarketAdjustment>,
        currency: Option<Currency>,
        session_type: Option<SessionType>,
        replay_session: Option<&str>,
    ) -> Result<()> {
        self.send(
            "resolve_symbol",
            &payload!(
                session,
                symbol_series_id,
                symbol_init()
                    .instrument(instrument)
                    .maybe_adjustment(adjustment)
                    .maybe_currency(currency)
                    .maybe_session_type(session_type)
                    .maybe_replay(replay_session)
                    .call()?
            ),
        )
        .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self), level = "debug")]
    pub async fn delete_chart_session(&self, session: &str) -> Result<()> {
        self.send("chart_delete_session", &payload!(session))
            .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self), level = "debug")]
    pub async fn request_more_data(
        &self,
        chart_session: &str,
        series_id: &str,
        num: u64,
    ) -> Result<()> {
        self.send(
            "request_more_data",
            &payload!(chart_session, series_id, num),
        )
        .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self), level = "debug")]
    pub async fn request_more_tickmarks(
        &self,
        chart_session: &str,
        series_id: &str,
        num: u64,
    ) -> Result<()> {
        self.send(
            "request_more_tickmarks",
            &payload!(chart_session, series_id, num),
        )
        .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self), level = "debug")]
    pub async fn create_replay_session(&self, replay_session: &str) -> Result<()> {
        self.send("replay_create_session", &payload!(replay_session))
            .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self), level = "debug")]
    #[builder]
    pub async fn add_replay_series(
        &self,
        replay_session: &str,
        request_id: &str,
        instrument: &str, // e.g., "HOSE:FPT"
        adjustment: Option<MarketAdjustment>,
        session_type: Option<SessionType>,
        currency: Option<Currency>,
        interval: Interval,
    ) -> Result<()> {
        let sym_init = symbol_init()
            .instrument(instrument)
            .maybe_adjustment(adjustment)
            .maybe_currency(currency)
            .maybe_session_type(session_type)
            .call()?;
        let payloads =
            build_add_replay_series_payload(replay_session, request_id, sym_init, interval);
        self.send("replay_add_series", &payloads).await?;
        Ok(())
    }

    #[tracing::instrument(skip(self), level = "debug")]
    pub async fn delete_replay_session(&self, replay_session: &str) -> Result<()> {
        self.send("replay_delete_session", &payload!(replay_session))
            .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self), level = "debug")]
    pub async fn replay_step(
        &self,
        replay_session: &str,
        request_id: &str,
        step: u64,
    ) -> Result<()> {
        let payloads = build_replay_step_payload(replay_session, request_id, step);
        self.send("replay_step", &payloads).await?;
        Ok(())
    }

    #[tracing::instrument(skip(self), level = "debug")]
    pub async fn replay_start(
        &self,
        replay_session: &str,
        request_id: &str,
        interval: u64,
    ) -> Result<()> {
        let payloads = build_replay_start_payload(replay_session, request_id, interval);
        self.send("replay_start", &payloads).await?;
        Ok(())
    }

    #[tracing::instrument(skip(self), level = "debug")]
    pub async fn replay_stop(&self, replay_session: &str, request_id: &str) -> Result<()> {
        let payloads = build_replay_stop_payload(replay_session, request_id);
        self.send("replay_stop", &payloads).await?;
        Ok(())
    }

    #[tracing::instrument(skip(self), level = "debug")]
    pub async fn replay_reset(
        &self,
        replay_session: &str,
        request_id: &str,
        timestamp: i64,
    ) -> Result<()> {
        let payloads = build_replay_reset_payload(replay_session, request_id, timestamp);
        self.send("replay_reset", &payloads).await?;
        Ok(())
    }

    #[tracing::instrument(skip(self), level = "debug")]
    #[builder]
    pub async fn create_study(
        &self,
        chart_session: &str,
        study_ids: &[&str; 2],
        chart_series_id: &str,
        study: StudyConfiguration,
    ) -> Result<()> {
        let mut payloads: Vec<Value> = vec![
            Value::from(chart_session),
            Value::from(study_ids[0]),
            Value::from(study_ids[1]),
            Value::from(chart_series_id),
        ];

        match study {
            StudyConfiguration::Pine(pine_indicator) => {
                payloads.push(Value::from(pine_indicator.script_type.to_string()));
                payloads.push(pine_indicator.to_study_inputs()?);
            }
            StudyConfiguration::Builtin(study_name, study_config) => {
                payloads.push(Value::from(study_name));
                payloads.push(json!(study_config));
            }
        }

        self.send("create_study", &payloads).await?;
        Ok(())
    }

    #[tracing::instrument(skip(self), level = "debug")]
    #[builder]
    pub async fn modify_study(
        &self,
        chart_session: &str,
        study_ids: &[&str; 2],
        study: StudyConfiguration,
    ) -> Result<()> {
        let inputs = match study {
            StudyConfiguration::Pine(pine_indicator) => pine_indicator.to_study_inputs()?,
            StudyConfiguration::Builtin(_study_name, study_config) => json!(study_config),
        };
        let payloads =
            build_modify_study_payload(chart_session, study_ids[0], study_ids[1], inputs);
        self.send("modify_study", &payloads).await?;
        Ok(())
    }

    #[tracing::instrument(skip(self), level = "debug")]
    pub async fn remove_study(&self, chart_session: &str, study_id: &str) -> Result<()> {
        self.send("remove_study", &payload!(chart_session, study_id))
            .await?;
        Ok(())
    }

    pub async fn delete(&self) -> Result<()> {
        // Close the socket last
        if let Err(e) = self.close().await {
            error!("Failed to close socket: {:?}", e);
            return Err(e);
        }

        debug!("WebSocket client deleted successfully");
        Ok(())
    }

    pub async fn subscribe(&self) -> Result<()> {
        let read = self.read.lock().await;
        if let Err(e) = self.event_loop(read).await {
            error!("Event loop failed: {}", e);
            self.is_closed.store(true, Ordering::Relaxed);
            self.cancellation.cancel();
            return Err(e);
        }
        Ok(())
    }

    pub async fn closed_notifier(&self) {
        self.cancellation.cancelled().await;
    }

    /// Fire-and-forget ping. Ignores `WouldBlock` when write buffer is full.
    pub async fn try_ping(&self) -> Result<()> {
        if self.is_closed() {
            return Ok(());
        }
        self.ping(&Message::Ping(Vec::new().into()))
            .await
            .map_err(|e| Error::WebSocket(ustr(&format!("{e}"))))?;
        Ok(())
    }

    pub async fn get_connection_stats(&self) -> Value {
        let health_metrics = self.health_metrics.read().await;
        let is_closed = self.is_closed();

        json!({
            "consecutive_errors": self.error_stats.get_consecutive_errors(),
            "total_errors": self.error_stats.total_errors.load(Ordering::SeqCst),
            "recovery_attempts": self.error_stats.recovery_attempts.load(Ordering::SeqCst),
            "connection_drops": self.error_stats.connection_drops.load(Ordering::SeqCst),
            "health": format!("{:?}", health_metrics.health),
            "avg_response_time_ms": health_metrics.avg_response_time.as_millis(),
            "circuit_breaker_open": self.circuit_breaker_open.load(Ordering::Relaxed),
            "is_closed": is_closed,
        })
    }
}

impl<T: Handler> Socket for WebSocketClient<T> {
    async fn event_loop(
        &self,
        mut read: MutexGuard<'_, SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>>,
    ) -> Result<()> {
        trace!("WebSocket event loop started");

        let mut ping_interval = tokio::time::interval(self.error_config.ping_interval);
        ping_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            if self.is_closed.load(Ordering::Relaxed) {
                trace!("WebSocket is closed, ending event loop");
                break;
            }

            tokio::select! {
                // Handle incoming messages
                message_result = timeout(Duration::from_secs(30), read.next()) => {
                    match message_result {
                        Ok(Some(Ok(message))) => {
                            trace!("Received message: {:?}", message);
                            if let Err(e) = self.handle_raw_messages(message).await {
                                self.handle_error(e, ustr("handle_raw_messages")).await?;
                            } else {
                                // Reset consecutive errors on successful message processing
                                if self.error_stats.get_consecutive_errors() > 0 {
                                    self.error_stats.reset_consecutive();
                                    debug!("Reset consecutive errors after successful message processing");
                                }
                                self.error_stats.update_last_successful_message().await;
                            }
                        }
                        Ok(Some(Err(e))) => {
                            error!("Error reading message: {:#?}", e);
                            self.error_stats.connection_drops.fetch_add(1, Ordering::SeqCst);
                            self.handle_error(
                                Error::WebSocket(e.to_string().into()),
                                ustr("event_loop_read"),
                            ).await?;

                            // For connection errors, we should break the loop
                            if e.to_string().contains("ConnectionClosed") ||
                               e.to_string().contains("ConnectionReset") {
                                break;
                            }
                        }
                        Ok(None) => {
                            info!("WebSocket stream ended");
                            self.is_closed.store(true, Ordering::Relaxed);
                            break;
                        }
                        Err(_) => {
                            warn!("WebSocket read timeout, checking connection health");
                            if let Err(e) = self.perform_health_check().await {
                                warn!("Health check failed during timeout: {}", e);
                                // Continue trying unless it's a fatal error
                                if matches!(self.classify_error_severity(&e, "health_check").await, ErrorSeverity::Fatal) {
                                    break;
                                }
                            }
                        }
                    }
                }

                // Periodic ping
                _ = ping_interval.tick() => {
                    if !self.is_closed() {
                        if let Err(e) = self.try_ping().await {
                            warn!("Periodic ping failed: {}", e);
                            self.handle_error(e, ustr("periodic_ping")).await?;
                        }
                    }
                }

                // Cancellation
                _ = self.cancellation.cancelled() => {
                    info!("Event loop cancelled");
                    break;
                }
            }
        }

        trace!("WebSocket event loop ended");
        Ok(())
    }

    async fn handle_raw_messages(&self, raw: Message) -> Result<()> {
        match &raw {
            Message::Text(text) => {
                trace!("Received text message: {}", text);
                self.handle_parsed_messages(parse_packet(text), &raw)
                    .await?;
            }
            Message::Close(msg) => {
                warn!("Connection closed with code: {:?}", msg);
                self.is_closed.store(true, Ordering::Relaxed);
                self.cancellation.cancel();
            }
            Message::Binary(msg) => {
                debug!("Received binary message: {:?}", msg);
                // TODO: handle binary messages
            }
            Message::Ping(msg) => {
                trace!("Received ping message: {:?}", msg);
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
        _raw: &Message,
    ) -> Result<()> {
        for message in messages {
            match message {
                SocketMessage::SocketServerInfo(info) => {
                    trace!("received server info: {:?}", info);
                }
                SocketMessage::SocketMessage(msg) => {
                    trace!(
                        "Processing socket message: method={}, params={:?}",
                        msg.m, msg.p
                    );
                    if let Err(e) = self.handle_message_data(msg).await {
                        self.handle_error(e, ustr("handle_message_data")).await?;
                    }
                }
                SocketMessage::Heartbeat(counter) => {
                    debug!("handling heartbeat message: {counter}");
                    let echo = message.heartbeat_echo().unwrap_or_else(|| {
                        let h = format!("~h~{counter}");
                        format!("~m~{}~m~{h}", h.len())
                    });
                    if let Err(e) = self.send_raw_message(&echo).await {
                        self.handle_error(e, ustr("heartbeat_echo")).await?;
                    }
                }
                SocketMessage::Other(value) => {
                    trace!("Received other message: {:?}", value);
                    if let Ok(server_info) = SocketServerInfo::deserialize(&value) {
                        info!("{}", server_info);
                    } else {
                        warn!("Received unrecognized message: {:?}", value);
                    }
                }
                SocketMessage::Unknown(s) => {
                    warn!("unknown message: {:?}", s);
                }
            }
        }
        Ok(())
    }

    #[tracing::instrument(skip(self), level = "trace")]
    async fn handle_message_data(&self, message: SocketMessageDe) -> Result<()> {
        dispatch_message_data(&self.handler, message);
        Ok(())
    }

    async fn handle_error(&self, error: Error, context: Ustr) -> Result<()> {
        let context_str = context.as_str();

        // Update error statistics
        let _consecutive_errors = self.error_stats.increment_error();
        self.error_stats.update_last_error_time().await;

        // Classify error severity with pattern detection
        let severity = self.classify_error_severity(&error, context_str).await;

        // Log the error appropriately
        self.log_error(&error, context_str, &severity);

        // Notify error handlers with enhanced context
        self.notify_error_handlers(&error, context_str, severity)
            .await;

        // Handle based on severity
        match severity {
            ErrorSeverity::Trace | ErrorSeverity::Minor => {
                // Continue without recovery
                Ok(())
            }

            ErrorSeverity::Moderate => {
                // Attempt soft recovery
                match self.attempt_error_recovery(severity, &error).await {
                    Ok(true) => Ok(()),
                    Ok(false) => {
                        warn!("Moderate error recovery failed, continuing anyway");
                        Ok(())
                    }
                    Err(recovery_err) => {
                        warn!("Error during moderate recovery: {}", recovery_err);
                        Ok(()) // Don't fail on moderate recovery errors
                    }
                }
            }

            ErrorSeverity::Critical | ErrorSeverity::Fatal => {
                // Attempt recovery or fail
                match self.attempt_error_recovery(severity, &error).await {
                    Ok(recovered) => {
                        if !recovered {
                            error!(
                                "Failed to recover from {} error",
                                if matches!(severity, ErrorSeverity::Critical) {
                                    "critical"
                                } else {
                                    "fatal"
                                }
                            );
                            return Err(Error::Internal(ustr("Error recovery failed")));
                        }
                        Ok(())
                    }
                    Err(recovery_err) => {
                        error!("Error during recovery attempt: {}", recovery_err);
                        Err(recovery_err)
                    }
                }
            }
        }
    }
}

/// Route a decoded socket message through the handler's event and quote callbacks.
pub fn dispatch_message_data<H: Handler>(handler: &H, message: SocketMessageDe) {
    let event = TradingViewDataEvent::from(message.m);
    handler.handle_events(event, &message.p);
    if event == TradingViewDataEvent::OnQuoteData {
        handler.handle_quote_data(&message.p);
    }
}

/// Build offline payload array for `modify_study`.
///
/// Returns `[chart_session, study_id, study_sub_id, inputs]`.
pub fn build_modify_study_payload(
    chart_session: &str,
    study_id: &str,
    study_sub_id: &str,
    inputs: Value,
) -> Vec<Value> {
    vec![
        Value::from(chart_session),
        Value::from(study_id),
        Value::from(study_sub_id),
        inputs,
    ]
}

/// Build offline payload array for `replay_start`.
///
/// Returns `[replay_session, request_id, interval_ms]`.
pub fn build_replay_start_payload(
    replay_session: &str,
    request_id: &str,
    interval_ms: u64,
) -> Vec<Value> {
    vec![
        Value::from(replay_session),
        Value::from(request_id),
        Value::from(interval_ms),
    ]
}

/// Build offline payload array for `replay_step`.
///
/// Returns `[replay_session, request_id, step]`.
pub fn build_replay_step_payload(replay_session: &str, request_id: &str, step: u64) -> Vec<Value> {
    vec![
        Value::from(replay_session),
        Value::from(request_id),
        Value::from(step),
    ]
}

/// Build offline payload array for `replay_stop`.
///
/// Returns `[replay_session, request_id]`.
pub fn build_replay_stop_payload(replay_session: &str, request_id: &str) -> Vec<Value> {
    vec![Value::from(replay_session), Value::from(request_id)]
}

/// Build offline payload array for `replay_reset`.
///
/// Returns `[replay_session, request_id, timestamp]`.
pub fn build_replay_reset_payload(
    replay_session: &str,
    request_id: &str,
    timestamp: i64,
) -> Vec<Value> {
    vec![
        Value::from(replay_session),
        Value::from(request_id),
        Value::from(timestamp),
    ]
}

/// Build offline payload array for `replay_add_series`.
///
/// Returns `[replay_session, request_id, symbol_init, timeframe]`.
pub fn build_add_replay_series_payload(
    replay_session: &str,
    request_id: &str,
    symbol_init: String,
    interval: Interval,
) -> Vec<Value> {
    vec![
        Value::from(replay_session),
        Value::from(request_id),
        Value::from(symbol_init),
        Value::from(interval.to_string()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    struct MockHandler {
        event_called: AtomicBool,
        quote_called: AtomicBool,
        quote_count: AtomicUsize,
    }

    impl MockHandler {
        fn new() -> Self {
            Self {
                event_called: AtomicBool::new(false),
                quote_called: AtomicBool::new(false),
                quote_count: AtomicUsize::new(0),
            }
        }
    }

    impl Handler for MockHandler {
        fn handle_events(&self, _event: TradingViewDataEvent, _message: &[Value]) {
            self.event_called.store(true, Ordering::SeqCst);
        }
        fn handle_quote_data(&self, _message: &[Value]) {
            self.quote_called.store(true, Ordering::SeqCst);
            self.quote_count.fetch_add(1, Ordering::SeqCst);
        }
        fn handle_series_data(&self, _event: TradingViewDataEvent, _messages: &[Value]) {}
        fn notify_error(&self, _error: Error, _message: &[Value]) {}
    }

    #[test]
    fn test_modify_study_payload_has_4_args() {
        let inputs = serde_json::json!({ "text": "//@version=5\nindicator('Test')" });
        let payload = build_modify_study_payload("cs_123", "st6", "st1", inputs.clone());

        assert_eq!(
            payload.len(),
            4,
            "modify_study payload length must be exactly 4"
        );
        assert_eq!(payload[0], Value::from("cs_123"));
        assert_eq!(payload[1], Value::from("st6"));
        assert_eq!(payload[2], Value::from("st1"));
        assert_eq!(payload[3], inputs);
    }

    #[test]
    fn test_replay_start_payload_interval_is_numeric_millis() {
        let payload = build_replay_start_payload("rs_100", "req_replay_1", 250);

        assert_eq!(payload.len(), 3, "replay_start payload length must be 3");
        assert_eq!(payload[0], Value::from("rs_100"));
        assert_eq!(payload[1], Value::from("req_replay_1"));
        assert!(
            payload[2].is_number(),
            "third argument must be numeric milliseconds"
        );
        assert_eq!(payload[2].as_u64(), Some(250));
    }

    #[test]
    fn test_replay_other_payloads() {
        let step_payload = build_replay_step_payload("rs_100", "req_step_1", 5);
        assert_eq!(step_payload.len(), 3);
        assert_eq!(step_payload[2], Value::from(5u64));

        let stop_payload = build_replay_stop_payload("rs_100", "req_stop_1");
        assert_eq!(stop_payload.len(), 2);
        assert_eq!(stop_payload[0], Value::from("rs_100"));
        assert_eq!(stop_payload[1], Value::from("req_stop_1"));

        let reset_payload = build_replay_reset_payload("rs_100", "req_reset_1", 1_700_000_000);
        assert_eq!(reset_payload.len(), 3);
        assert_eq!(reset_payload[2], Value::from(1_700_000_000i64));

        let add_payload = build_add_replay_series_payload(
            "rs_100",
            "req_add_1",
            "={\"symbol\":\"NASDAQ:AAPL\"}".to_string(),
            Interval::OneDay,
        );
        assert_eq!(add_payload.len(), 4);
        assert_eq!(add_payload[3], Value::from("1D"));
    }

    #[test]
    fn test_heartbeat_echo_framed_format() {
        let msg = SocketMessage::<SocketMessageDe>::Heartbeat(42);
        assert_eq!(msg.heartbeat_echo(), Some("~m~5~m~~h~42".to_string()));
    }

    #[test]
    fn test_quote_data_handler_routing() {
        let handler = MockHandler::new();
        let quote_msg = SocketMessageDe {
            m: ustr::ustr("qsd"),
            p: vec![
                serde_json::json!("quote_session_1"),
                serde_json::json!({"s": "ok"}),
            ],
            t: 0,
            t_ms: 0,
        };

        dispatch_message_data(&handler, quote_msg);
        assert!(
            handler.event_called.load(Ordering::SeqCst),
            "handle_events must be called"
        );
        assert!(
            handler.quote_called.load(Ordering::SeqCst),
            "handle_quote_data must be called for qsd"
        );
        assert_eq!(handler.quote_count.load(Ordering::SeqCst), 1);

        let non_quote_msg = SocketMessageDe {
            m: ustr::ustr("timescale_update"),
            p: vec![serde_json::json!("cs_1")],
            t: 0,
            t_ms: 0,
        };
        dispatch_message_data(&handler, non_quote_msg);
        assert_eq!(
            handler.quote_count.load(Ordering::SeqCst),
            1,
            "quote_data should not be called for non-qsd"
        );
    }
}
