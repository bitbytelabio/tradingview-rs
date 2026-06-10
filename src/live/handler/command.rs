use core::fmt;
use serde::{Deserialize, Serialize};
use std::{collections::VecDeque, sync::Arc};
use tokio::{
    select,
    task::JoinHandle,
    time::{Duration, Instant, interval, sleep, timeout},
};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, instrument, warn};

use crate::{
    Error, Result,
    error::TradingViewError,
    live::handler::{CommandRx, Handler, message::*},
    websocket::WebSocketClient,
};

/// Priority level for queued commands.
///
/// Higher-priority commands are dispatched before lower-priority ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CommandPriority {
    Critical = 3,
    High = 2,
    Normal = 1,
    Low = 0,
}

/// A command to be dispatched to the TradingView WebSocket session.
///
/// Covers all protocol operations: chart sessions, quote subscriptions,
/// replay mode, Pine Script studies, and connection management.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Command {
    Close,
    Ping,
    SendRawMessage(CommandMsg),
    SetAuthToken(CommandMsg),
    SetLocale(SetLocaleCommandMsg),
    SetDataQuality(CommandMsg),
    SetTimeZone(SetTimeZoneCommandMsg),

    /// Quote Session Commands
    CreateQuoteSession(CommandMsg),
    DeleteQuoteSession(CommandMsg),
    FastSymbols(QuoteCommandMsg),
    SetQuoteFields(CommandMsg),
    AddQuoteSymbols(QuoteCommandMsg),
    RemoveQuoteSymbols(QuoteCommandMsg),

    /// Chart Session Commands
    CreateChartSession(CommandMsg),
    DeleteChartSession(CommandMsg),
    RequestMoreData(ChartDataRequestMsg),
    RequestMoreTickmarks(ChartDataRequestMsg),
    CreateChartSeries(ChartSeriesCommandMsg),
    ModifyChartSeries(ChartSeriesCommandMsg),
    RemoveSeries(SessionTerminationCommandMsg),
    ResolveSymbol(ResolveSymbolCommandMsg),

    /// Replay Session Commands
    CreateReplaySession(CommandMsg),
    DeleteReplaySession(CommandMsg),
    AddReplaySeries(AddReplaySeriesCommandMsg),
    ReplayStep(ReplayStepCommandMsg),
    ReplayStart(ReplayStartCommandMsg),
    ReplayStop(SessionTerminationCommandMsg),
    ReplayReset(ReplayResetCommandMsg),

    /// Study Commands
    CreateStudy(StudyCommandMsg),
    ModifyStudy(StudyCommandMsg),
    RemoveStudy(SessionTerminationCommandMsg),

    /// Batch Commands
    BatchCommands(Vec<Command>),

    /// Conditional Commands
    ConditionalCommand {
        condition: CommandCondition,
        command: Box<Command>,
        fallback: Option<Box<Command>>,
    },
}

impl Command {
    /// Validate command parameters
    pub fn validate(&self) -> Result<()> {
        match self {
            Command::CreateQuoteSession(msg)
            | Command::CreateChartSession(msg)
            | Command::CreateReplaySession(msg) => {
                if msg.inner.is_empty() {
                    return Err(Error::Internal("Session name cannot be empty".into()));
                }
            }
            Command::AddQuoteSymbols(msg)
            | Command::RemoveQuoteSymbols(msg)
            | Command::FastSymbols(msg) => {
                if msg.symbols.is_empty() {
                    return Err(Error::Internal("Symbol list cannot be empty".into()));
                }
                if msg.quote_session.is_empty() {
                    return Err(Error::Internal("Quote session cannot be empty".into()));
                }
            }
            Command::CreateChartSeries(msg) | Command::ModifyChartSeries(msg) => {
                if msg.chart_session.is_empty()
                    || msg.series_id.is_empty()
                    || msg.symbol_series_id.is_empty()
                {
                    return Err(Error::Internal(
                        "Chart series parameters cannot be empty".into(),
                    ));
                }
                if msg.bar_count == 0 {
                    return Err(Error::Internal("Bar count must be greater than 0".into()));
                }
            }
            Command::BatchCommands(commands) => {
                if commands.is_empty() {
                    return Err(Error::Internal("Batch commands cannot be empty".into()));
                }
                if commands.len() > 100 {
                    return Err(Error::Internal("Batch size too large (max 100)".into()));
                }
                for cmd in commands {
                    cmd.validate()?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Get command priority for queue management
    pub fn priority(&self) -> CommandPriority {
        match self {
            Command::Close | Command::SetAuthToken(_) => CommandPriority::Critical,
            Command::Ping => CommandPriority::High,
            Command::CreateQuoteSession(_)
            | Command::CreateChartSession(_)
            | Command::CreateReplaySession(_) => CommandPriority::High,
            Command::BatchCommands(_) => CommandPriority::Normal,
            Command::ConditionalCommand { .. } => CommandPriority::Normal,
            _ => CommandPriority::Normal,
        }
    }

    /// Get estimated execution time for timeout management
    pub fn estimated_duration(&self) -> Duration {
        match self {
            Command::Ping => Duration::from_secs(1),
            Command::CreateChartSeries(_)
            | Command::ModifyChartSeries(_)
            | Command::ResolveSymbol(_) => Duration::from_secs(5),
            Command::BatchCommands(commands) => Duration::from_millis(commands.len() as u64 * 100),
            _ => Duration::from_secs(3),
        }
    }

    /// Check if command requires session to exist
    pub fn requires_session(&self) -> Option<&str> {
        match self {
            Command::DeleteQuoteSession(msg) | Command::SetQuoteFields(msg) => Some(&msg.inner),
            Command::AddQuoteSymbols(msg)
            | Command::RemoveQuoteSymbols(msg)
            | Command::FastSymbols(msg) => Some(&msg.quote_session),
            Command::DeleteChartSession(msg) => Some(&msg.inner),
            Command::CreateChartSeries(msg) | Command::ModifyChartSeries(msg) => {
                Some(&msg.chart_session)
            }
            Command::RemoveSeries(msg) | Command::RemoveStudy(msg) => Some(&msg.chart_session),
            _ => None,
        }
    }
}

/// Connection state tracking with timestamps for better monitoring
#[derive(Debug, Clone, PartialEq, Copy, Eq)]
pub struct ConnectionState {
    pub status: ConnectionStatus,
    pub last_change: Instant,
    pub last_successful_operation: Option<Instant>,
}

/// Current state of the WebSocket connection.
#[derive(Debug, Clone, PartialEq, Copy, Eq)]
pub enum ConnectionStatus {
    Connected,
    Disconnected,
    Reconnecting,
    Shutdown,
}

impl ConnectionState {
    fn new(status: ConnectionStatus) -> Self {
        Self {
            status,
            last_change: Instant::now(),
            last_successful_operation: None,
        }
    }

    fn transition_to(&mut self, new_status: ConnectionStatus) {
        if self.status != new_status {
            self.status = new_status;
            self.last_change = Instant::now();
        }
    }

    fn mark_successful_operation(&mut self) {
        self.last_successful_operation = Some(Instant::now());
    }

    fn time_since_last_success(&self) -> Option<Duration> {
        self.last_successful_operation.map(|t| t.elapsed())
    }
}

/// Error classification for better handling
#[derive(Debug, Clone, PartialEq, Copy, Eq)]
enum ErrorSeverity {
    /// Temporary errors that should trigger reconnection
    Recoverable,
    /// Permanent errors that should stop the runner
    Fatal,
    /// Command-specific errors that don't affect connection
    CommandOnly,
}

#[derive(Debug, Clone, Copy)]
struct ExponentialBackoff {
    config: BackoffConfig,
    current_delay: Duration,
    attempts: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct BackoffConfig {
    initial_delay: Duration,
    max_delay: Duration,
    max_attempts: usize,
    multiplier: f64,
    jitter_percent: f64,
}

impl Default for BackoffConfig {
    fn default() -> Self {
        Self {
            initial_delay: Duration::from_millis(1000),
            max_delay: Duration::from_secs(60),
            max_attempts: 10,
            multiplier: 2.0,
            jitter_percent: 0.1,
        }
    }
}

impl ExponentialBackoff {
    fn new(config: BackoffConfig) -> Self {
        Self {
            current_delay: config.initial_delay,
            config,
            attempts: 0,
        }
    }

    fn next_backoff(&mut self) -> Option<Duration> {
        if self.attempts >= self.config.max_attempts {
            return None;
        }

        let mut delay = self.current_delay;
        self.attempts += 1;

        // Add jitter to prevent thundering herd problem
        let jitter_range = delay.as_millis() as f64 * self.config.jitter_percent;
        let jitter = (rand::random::<f64>() - 0.5) * 2.0 * jitter_range;
        delay = Duration::from_millis((delay.as_millis() as f64 + jitter).max(0.0) as u64);

        // Exponential backoff with cap
        self.current_delay = std::cmp::min(
            Duration::from_millis(
                (self.current_delay.as_millis() as f64 * self.config.multiplier) as u64,
            ),
            self.config.max_delay,
        );

        Some(delay)
    }

    fn reset(&mut self) {
        self.current_delay = self.config.initial_delay;
        self.attempts = 0;
    }

    fn remaining_attempts(&self) -> usize {
        self.config.max_attempts.saturating_sub(self.attempts)
    }
}

#[derive(Debug, Clone)]
pub struct CommandQueue {
    critical_queue: VecDeque<Command>,
    high_queue: VecDeque<Command>,
    normal_queue: VecDeque<Command>,
    low_queue: VecDeque<Command>,
    max_size: usize,
    dropped_count: u64,
    session_tracker: std::collections::HashSet<String>,
}

impl CommandQueue {
    fn new(max_size: usize) -> Self {
        Self {
            critical_queue: VecDeque::new(),
            high_queue: VecDeque::new(),
            normal_queue: VecDeque::new(),
            low_queue: VecDeque::new(),
            max_size,
            dropped_count: 0,
            session_tracker: std::collections::HashSet::new(),
        }
    }

    fn enqueue(&mut self, cmd: Command) -> Result<()> {
        // Validate command first
        cmd.validate()?;

        // Check if command requires existing session
        if let Some(session) = cmd.requires_session() {
            if !self.session_tracker.contains(session) {
                return Err(Error::Internal(
                    format!("Session '{}' does not exist", session).into(),
                ));
            }
        }

        // Track session creation/deletion
        match &cmd {
            Command::CreateQuoteSession(msg)
            | Command::CreateChartSession(msg)
            | Command::CreateReplaySession(msg) => {
                self.session_tracker.insert(msg.inner.to_string());
            }
            Command::DeleteQuoteSession(msg)
            | Command::DeleteChartSession(msg)
            | Command::DeleteReplaySession(msg) => {
                self.session_tracker.remove(&msg.inner.to_string());
            }
            _ => {}
        }

        // Check capacity and drop if necessary
        if self.total_len() >= self.max_size {
            // Try to drop from lower priority queues first
            if self.drop_lowest_priority() {
                self.dropped_count += 1;
                warn!(
                    "Dropped command due to queue overflow (total dropped: {})",
                    self.dropped_count
                );
            } else {
                return Err(Error::Internal("Command queue is full".into()));
            }
        }

        let queue = match cmd.priority() {
            CommandPriority::Critical => &mut self.critical_queue,
            CommandPriority::High => &mut self.high_queue,
            CommandPriority::Normal => &mut self.normal_queue,
            CommandPriority::Low => &mut self.low_queue,
        };

        queue.push_back(cmd);
        Ok(())
    }

    fn drop_lowest_priority(&mut self) -> bool {
        if let Some(_) = self.low_queue.pop_front() {
            return true;
        }
        if let Some(_) = self.normal_queue.pop_front() {
            return true;
        }
        if let Some(_) = self.high_queue.pop_front() {
            return true;
        }
        false
    }

    fn dequeue(&mut self) -> Option<Command> {
        self.critical_queue
            .pop_front()
            .or_else(|| self.high_queue.pop_front())
            .or_else(|| self.normal_queue.pop_front())
            .or_else(|| self.low_queue.pop_front())
    }

    fn drain(&mut self) -> Vec<Command> {
        let mut commands = Vec::with_capacity(self.total_len());

        while let Some(cmd) = self.dequeue() {
            commands.push(cmd);
        }

        commands
    }

    fn total_len(&self) -> usize {
        self.critical_queue.len()
            + self.high_queue.len()
            + self.normal_queue.len()
            + self.low_queue.len()
    }

    fn is_empty(&self) -> bool {
        self.total_len() == 0
    }

    fn clear(&mut self) {
        self.critical_queue.clear();
        self.high_queue.clear();
        self.normal_queue.clear();
        self.low_queue.clear();
        self.session_tracker.clear();
    }

    fn detailed_stats(&self) -> CommandQueueStats {
        CommandQueueStats {
            critical_queue_len: self.critical_queue.len(),
            high_queue_len: self.high_queue.len(),
            normal_queue_len: self.normal_queue.len(),
            low_queue_len: self.low_queue.len(),
            total_len: self.total_len(),
            max_capacity: self.max_size,
            dropped_count: self.dropped_count,
            active_sessions: self.session_tracker.len(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CommandQueueStats {
    pub critical_queue_len: usize,
    pub high_queue_len: usize,
    pub normal_queue_len: usize,
    pub low_queue_len: usize,
    pub total_len: usize,
    pub max_capacity: usize,
    pub dropped_count: u64,
    pub active_sessions: usize,
}

impl fmt::Display for CommandQueueStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Queue: C:{} H:{} N:{} L:{} | Total:{}/{} | Sessions:{} | Dropped:{}",
            self.critical_queue_len,
            self.high_queue_len,
            self.normal_queue_len,
            self.low_queue_len,
            self.total_len,
            self.max_capacity,
            self.active_sessions,
            self.dropped_count
        )
    }
}

#[derive(Debug, Default, Clone)]
pub struct ConnectionStats {
    pub reconnect_attempts: u64,
    pub successful_reconnects: u64,
    pub commands_processed: u64,
    pub commands_failed: u64,
    pub errors_handled: u64,
    pub total_uptime: Duration,
    pub last_reconnect_duration: Option<Duration>,
    pub average_command_latency: Option<Duration>,
}

impl ConnectionStats {
    fn record_command_success(&mut self, duration: Duration) {
        self.commands_processed += 1;
        self.average_command_latency = Some(
            self.average_command_latency
                .map(|avg| (avg + duration) / 2)
                .unwrap_or(duration),
        );
    }

    fn record_command_failure(&mut self) {
        self.commands_failed += 1;
    }

    fn record_reconnect_attempt(&mut self) {
        self.reconnect_attempts += 1;
    }

    fn record_successful_reconnect(&mut self, duration: Duration) {
        self.successful_reconnects += 1;
        self.last_reconnect_duration = Some(duration);
    }

    fn success_rate(&self) -> f64 {
        let total = self.commands_processed + self.commands_failed;
        if total == 0 {
            return 1.0;
        }
        self.commands_processed as f64 / total as f64
    }
}

/// Enhanced configuration for CommandRunner
#[derive(Debug, Clone)]
pub struct CommandRunnerConfig {
    pub heartbeat_interval: Duration,
    pub health_check_interval: Duration,
    pub command_timeout: Duration,
    pub reconnect_timeout: Duration,
    pub max_queue_size: usize,
    pub backoff_config: BackoffConfig,
    pub health_check_timeout: Duration,
}

impl Default for CommandRunnerConfig {
    fn default() -> Self {
        Self {
            heartbeat_interval: Duration::from_secs(30),
            health_check_interval: Duration::from_secs(5),
            command_timeout: Duration::from_secs(10),
            reconnect_timeout: Duration::from_secs(30),
            max_queue_size: 100,
            backoff_config: BackoffConfig::default(),
            health_check_timeout: Duration::from_secs(60),
        }
    }
}

/// Central command dispatcher for the WebSocket session.
///
/// Reads [`Command`]s from an MPSC channel and dispatches them to the
/// [`WebSocketClient`]. Manages connection lifecycle, reconnection, and
/// error recovery.
///
/// Spawn via [`CommandRunner::run()`] — it blocks until shutdown is triggered.
///
/// [`Command`]: crate::live::handler::command::Command
/// [`WebSocketClient`]: crate::websocket::WebSocketClient
/// [`CommandRunner::run()`]: CommandRunner::run()
pub struct CommandRunner<T: Handler> {
    rx: CommandRx,
    ws: Arc<WebSocketClient<T>>,
    shutdown: CancellationToken,
    state: ConnectionState,
    command_queue: CommandQueue,
    stats: ConnectionStats,
    reader_handle: Option<JoinHandle<()>>,
    config: CommandRunnerConfig,
    start_time: Instant,
}

impl<T: Handler> CommandRunner<T> {
    pub fn new(rx: CommandRx, ws: Arc<WebSocketClient<T>>) -> Self {
        Self::with_config(rx, ws, CommandRunnerConfig::default())
    }

    pub fn with_config(
        rx: CommandRx,
        ws: Arc<WebSocketClient<T>>,
        config: CommandRunnerConfig,
    ) -> Self {
        Self {
            rx,
            ws,
            shutdown: CancellationToken::new(),
            state: ConnectionState::new(ConnectionStatus::Connected),
            command_queue: CommandQueue::new(config.max_queue_size),
            stats: ConnectionStats::default(),
            reader_handle: None,
            config,
            start_time: Instant::now(),
        }
    }

    // #[instrument(skip(self), fields(runner_id = %std::ptr::addr_of!(*self) as usize))]
    pub async fn run(mut self) -> Result<()> {
        let mut hb = interval(self.config.heartbeat_interval);
        let mut backoff = ExponentialBackoff::new(self.config.backoff_config);
        let mut health_check = interval(self.config.health_check_interval);
        let mut stats_timer = interval(Duration::from_secs(60)); // Log stats every minute

        info!("CommandRunner started with config: {:?}", self.config);

        // Initialize connection
        if let Err(e) = self.initialize_connection().await {
            error!("Failed to initialize connection: {}", e);
            self.state.transition_to(ConnectionStatus::Disconnected);
        }

        loop {
            select! {
                biased;

                _ = self.shutdown.cancelled() => {
                    info!("Shutdown signal received");
                    self.state.transition_to(ConnectionStatus::Shutdown);
                    break;
                },

                cmd = self.rx.recv() => match cmd {
                    Some(cmd) => {
                        let start = Instant::now();
                        match self.handle_command(cmd, &mut backoff).await {
                            Ok(_) => {
                                self.stats.record_command_success(start.elapsed());
                                self.state.mark_successful_operation();
                            },
                            Err(e) => {
                                self.stats.record_command_failure();
                                self.stats.errors_handled += 1;
                                self.handle_command_error(e).await;
                            }
                        }
                    },
                    None => {
                        info!("All command senders dropped, shutting down");
                        break;
                    }
                },

                _ = health_check.tick() => {
                    // Only perform health check if we're supposed to be connected
                    if matches!(self.state.status, ConnectionStatus::Connected | ConnectionStatus::Reconnecting) {
                        if let Err(e) = self.perform_health_check().await {
                            warn!("Health check failed: {}", e);
                            self.state.transition_to(ConnectionStatus::Disconnected);
                        } else {
                            self.state.mark_successful_operation();
                        }
                    }
                },

                _ = hb.tick() => {
                    if self.state.status == ConnectionStatus::Connected {
                        if let Err(e) = self.send_heartbeat().await {
                            warn!("Heartbeat failed: {}", e);
                            self.state.transition_to(ConnectionStatus::Disconnected);
                        } else {
                            self.state.mark_successful_operation();
                        }
                    }
                },

                _ = stats_timer.tick() => {
                    self.log_stats();
                },
            }

            // Handle disconnection state
            if self.state.status == ConnectionStatus::Disconnected {
                if let Err(e) = self.handle_reconnection(&mut backoff).await {
                    error!("Reconnection failed: {}", e);
                    break;
                }
            }
        }

        self.cleanup().await;

        Ok(())
    }

    /// Comprehensive health check that actively tests the connection
    async fn perform_health_check(&self) -> Result<()> {
        // First check basic connection state
        if self.ws.is_closed() {
            return Err(Error::Internal("WebSocket is closed".into()));
        }

        // Check if connection has been unhealthy for too long
        if let Some(time_since_success) = self.state.time_since_last_success() {
            if time_since_success > self.config.health_check_timeout {
                return Err(Error::Internal(
                    format!("No successful operations for {time_since_success:?}").into(),
                ));
            }
        }

        // Actively test the connection with a ping
        // Use a shorter timeout for health check pings
        let health_check_timeout = Duration::from_secs(5);
        match timeout(health_check_timeout, self.ws.try_ping()).await {
            Ok(Ok(_)) => {
                debug!("Health check ping successful");
                Ok(())
            }
            Ok(Err(e)) => {
                warn!("Health check ping failed: {}", e);
                Err(Error::Internal(
                    format!("Health check ping failed: {e}").into(),
                ))
            }
            Err(_) => {
                warn!(
                    "Health check ping timed out after {:?}",
                    health_check_timeout
                );
                Err(Error::Internal("Health check ping timeout".into()))
            }
        }
    }

    #[instrument(skip(self))]
    async fn initialize_connection(&mut self) -> Result<()> {
        // Start the WebSocket reader task
        self.start_reader_task();

        // Send initial authentication if we have a token
        let auth_token = *self.ws.auth_token.read().await;
        if auth_token != "unauthorized_user_token" {
            info!("Sending initial authentication token");
            self.ws.set_auth_token(&auth_token).await?;
        }

        self.state.mark_successful_operation();
        Ok(())
    }

    async fn handle_command_error(&mut self, error: Error) {
        match self.classify_error(&error) {
            ErrorSeverity::Fatal => {
                error!("Fatal error, shutting down: {}", error);
                self.state.transition_to(ConnectionStatus::Shutdown);
            }
            ErrorSeverity::Recoverable => {
                warn!("Recoverable error, will attempt reconnection: {}", error);
                self.state.transition_to(ConnectionStatus::Disconnected);
            }
            ErrorSeverity::CommandOnly => {
                warn!("Command error (continuing): {}", error);
            }
        }
    }

    fn start_reader_task(&mut self) {
        if self.reader_handle.is_none() {
            let ws = Arc::clone(&self.ws);
            let shutdown = self.shutdown.clone();

            self.reader_handle = Some(tokio::spawn(async move {
                info!("Starting WebSocket reader task");

                loop {
                    select! {
                        _ = shutdown.cancelled() => {
                            info!("WebSocket reader task shutting down");
                            break;
                        },
                        result = ws.subscribe() => {
                            match result {
                                Ok(_) => {
                                    info!("WebSocket reader task completed successfully");
                                    break;
                                },
                                Err(e) => {
                                    error!("WebSocket reader task failed: {}", e);
                                    // Small delay before retrying
                                    sleep(Duration::from_secs(1)).await;
                                }
                            }
                        }
                    }
                }
            }));
        }
    }

    #[instrument(skip(self, cmd, backoff))]
    async fn handle_command(
        &mut self,
        cmd: Command,
        backoff: &mut ExponentialBackoff,
    ) -> Result<()> {
        match self.state.status {
            ConnectionStatus::Connected => self.process_command(cmd).await,
            ConnectionStatus::Reconnecting => {
                if let Ok(()) = self.command_queue.enqueue(cmd) {
                    let stats = self.command_queue.detailed_stats();
                    info!("Command queued during reconnection: {}", stats);
                }
                Ok(())
            }
            ConnectionStatus::Disconnected => {
                if self.is_critical_command(&cmd) {
                    warn!(
                        "Critical command received while disconnected, attempting immediate reconnection"
                    );
                    self.handle_reconnection(backoff).await?;
                    self.process_command(cmd).await
                } else {
                    self.command_queue.enqueue(cmd)?;
                    Ok(())
                }
            }
            ConnectionStatus::Shutdown => {
                warn!("Ignoring command after shutdown");
                Ok(())
            }
        }
    }

    #[instrument(skip(self, cmd), fields(command_type = ?std::mem::discriminant(&cmd)))]
    async fn process_command(&mut self, cmd: Command) -> Result<()> {
        // Validate command before processing
        cmd.validate()?;

        // Use an iterative approach for nested commands
        let mut command_stack = VecDeque::new();
        command_stack.push_back(cmd);

        while let Some(current_cmd) = command_stack.pop_front() {
            match current_cmd {
                Command::BatchCommands(commands) => {
                    // Add all batch commands to the front of the stack in reverse order
                    // so they're processed in the correct order
                    for cmd in commands.into_iter().rev() {
                        command_stack.push_front(cmd);
                    }
                    continue;
                }
                Command::ConditionalCommand {
                    condition,
                    command,
                    fallback,
                } => {
                    let condition_met = self.evaluate_condition(&condition).await;

                    if condition_met {
                        debug!("Condition met, executing primary command");
                        command_stack.push_front(*command);
                    } else if let Some(fallback_cmd) = fallback {
                        debug!("Condition not met, executing fallback command");
                        command_stack.push_front(*fallback_cmd);
                    } else {
                        debug!("Condition not met and no fallback provided");
                    }
                    continue;
                }
                _ => {
                    // Process regular command
                    self.execute_single_command(current_cmd).await?;
                }
            }
        }

        Ok(())
    }

    async fn execute_single_command(&mut self, cmd: Command) -> Result<()> {
        // Use dynamic timeout based on command type
        let timeout_duration = cmd.estimated_duration().mul_f32(1.5); // 50% buffer

        let result = timeout(timeout_duration, async {
            match cmd {
                Command::Close => self.ws.close().await,
                Command::Ping => self.ws.try_ping().await,
                Command::SetAuthToken(auth_token) => {
                    self.ws.set_auth_token(&auth_token.inner).await
                }
                Command::CreateQuoteSession(session) => {
                    self.ws.create_quote_session(&session.inner).await
                }
                Command::SetLocale(locale) => {
                    self.ws.set_locale(&locale.language, &locale.country).await
                }
                Command::SetDataQuality(quality) => self.ws.set_data_quality(&quality.inner).await,
                Command::SetTimeZone(timezone) => {
                    self.ws
                        .set_timezone(&timezone.chart_session, timezone.timezone)
                        .await
                }
                Command::CreateChartSession(session) => {
                    self.ws.create_chart_session(&session.inner).await
                }
                Command::DeleteChartSession(session) => {
                    self.ws.delete_chart_session(&session.inner).await
                }
                Command::RequestMoreData(request) => {
                    self.ws
                        .request_more_data(&request.chart_session, &request.series_id, request.num)
                        .await
                }
                Command::RequestMoreTickmarks(request) => {
                    self.ws
                        .request_more_tickmarks(
                            &request.chart_session,
                            &request.series_id,
                            request.num,
                        )
                        .await
                }
                Command::SendRawMessage(command_msg) => {
                    self.ws.send_raw_message(&command_msg.inner).await
                }
                Command::DeleteQuoteSession(session) => {
                    self.ws.delete_quote_session(&session.inner).await
                }
                Command::FastSymbols(quote_command_msg) => {
                    self.ws
                        .fast_symbols(
                            &quote_command_msg.quote_session,
                            &quote_command_msg
                                .symbols
                                .iter()
                                .map(|s| s.as_str())
                                .collect::<Vec<_>>(),
                        )
                        .await
                }
                Command::SetQuoteFields(command_msg) => {
                    self.ws.set_fields(&command_msg.inner).await
                }
                Command::AddQuoteSymbols(quote_command_msg) => {
                    self.ws
                        .add_symbols(
                            &quote_command_msg.quote_session,
                            &quote_command_msg
                                .symbols
                                .iter()
                                .map(|s| s.as_str())
                                .collect::<Vec<_>>(),
                        )
                        .await
                }
                Command::RemoveQuoteSymbols(quote_command_msg) => {
                    self.ws
                        .remove_symbols(
                            &quote_command_msg.quote_session,
                            &quote_command_msg
                                .symbols
                                .iter()
                                .map(|s| s.as_str())
                                .collect::<Vec<_>>(),
                        )
                        .await
                }
                Command::CreateChartSeries(chart_series_command_msg) => {
                    self.ws
                        .create_series()
                        .chart_session(&chart_series_command_msg.chart_session)
                        .series_identifier(&chart_series_command_msg.series_identifier)
                        .series_id(&chart_series_command_msg.series_id)
                        .symbol_series_id(&chart_series_command_msg.symbol_series_id)
                        .interval(chart_series_command_msg.interval)
                        .bar_count(chart_series_command_msg.bar_count)
                        .maybe_range(chart_series_command_msg.range)
                        .call()
                        .await
                }
                Command::ModifyChartSeries(command) => {
                    self.ws
                        .modify_series()
                        .chart_session(&command.chart_session)
                        .series_identifier(&command.series_identifier)
                        .series_id(&command.series_id)
                        .symbol_series_id(&command.symbol_series_id)
                        .interval(command.interval)
                        .bar_count(command.bar_count)
                        .maybe_range(command.range)
                        .call()
                        .await
                }
                Command::RemoveSeries(command) => {
                    self.ws
                        .remove_series(&command.chart_session, &command.id)
                        .await
                }
                Command::ResolveSymbol(command) => {
                    self.ws
                        .resolve_symbol()
                        .session(&command.session)
                        .symbol_series_id(&command.symbol_series_id)
                        .maybe_adjustment(command.adjustment)
                        .maybe_currency(command.currency)
                        .maybe_session_type(command.session_type)
                        .maybe_replay_session(command.replay_session.as_deref())
                        .instrument(&command.instrument)
                        .call()
                        .await
                }
                Command::CreateReplaySession(command_msg) => {
                    self.ws.create_replay_session(&command_msg.inner).await
                }
                Command::DeleteReplaySession(command) => {
                    self.ws.delete_replay_session(&command.inner).await
                }
                Command::AddReplaySeries(command) => {
                    self.ws
                        .add_replay_series()
                        .maybe_adjustment(command.adjustment)
                        .maybe_currency(command.currency)
                        .maybe_session_type(command.session_type)
                        .chart_session(&command.chart_session)
                        .series_id(&command.series_id)
                        .interval(command.interval)
                        .instrument(&command.instrument)
                        .call()
                        .await
                }
                Command::ReplayStep(command) => {
                    self.ws
                        .replay_step(
                            &command.chart_session,
                            &command.series_id,
                            command.step as u64,
                        )
                        .await
                }
                Command::ReplayStart(command) => {
                    self.ws
                        .replay_start(&command.chart_session, &command.series_id, command.interval)
                        .await
                }
                Command::ReplayStop(command) => {
                    self.ws
                        .replay_stop(&command.chart_session, &command.id)
                        .await
                }
                Command::ReplayReset(command) => {
                    self.ws
                        .replay_reset(
                            &command.chart_session,
                            &command.series_id,
                            command.timestamp,
                        )
                        .await
                }
                Command::CreateStudy(command) => {
                    self.ws
                        .create_study()
                        .chart_session(&command.chart_session)
                        .study_ids(
                            &command
                                .study_ids
                                .iter()
                                .map(|s| s.as_str())
                                .collect::<Vec<_>>()
                                .try_into()
                                .unwrap(),
                        )
                        .chart_series_id(&command.chart_series_id)
                        .study(command.study.clone())
                        .call()
                        .await
                }
                Command::ModifyStudy(command) => {
                    self.ws
                        .modify_study()
                        .chart_session(&command.chart_session)
                        .study_ids(
                            &command
                                .study_ids
                                .iter()
                                .map(|s| s.as_str())
                                .collect::<Vec<_>>()
                                .try_into()
                                .expect("Study IDs must be exactly 2"),
                        )
                        .chart_series_id(&command.chart_series_id)
                        .study(command.study.clone())
                        .call()
                        .await
                }
                Command::RemoveStudy(session_termination_command_msg) => {
                    self.ws
                        .remove_study(
                            &session_termination_command_msg.chart_session,
                            &session_termination_command_msg.id,
                        )
                        .await
                }
                _ => Ok(()),
            }
        })
        .await;

        result.unwrap_or_else(|_| Err(Error::Internal("Command timeout".into())))
    }

    async fn evaluate_condition(&self, condition: &CommandCondition) -> bool {
        match condition {
            CommandCondition::SessionExists(session) => self
                .command_queue
                .session_tracker
                .contains(session.as_str()),
            CommandCondition::SymbolResolved(_symbol) => {
                // This would require maintaining state of resolved symbols
                true // Placeholder
            }
            CommandCondition::ConnectionHealthy => {
                !self.ws.is_closed() && matches!(self.state.status, ConnectionStatus::Connected)
            }
            CommandCondition::QueueEmpty => self.command_queue.is_empty(),
        }
    }

    fn is_critical_command(&self, cmd: &Command) -> bool {
        matches!(
            cmd,
            Command::SetAuthToken { .. }
                | Command::Close
                | Command::Ping
                | Command::CreateQuoteSession { .. }
                | Command::CreateChartSession { .. }
                | Command::CreateReplaySession { .. }
        )
    }

    #[instrument(skip(self, backoff))]
    async fn handle_reconnection(&mut self, backoff: &mut ExponentialBackoff) -> Result<()> {
        if self.state.status == ConnectionStatus::Shutdown {
            return Ok(());
        }

        self.state.transition_to(ConnectionStatus::Reconnecting);
        let reconnect_start = Instant::now();

        // Stop the old reader task gracefully
        self.stop_reader_task().await;

        while let Some(delay) = backoff.next_backoff() {
            self.stats.record_reconnect_attempt();

            info!(
                "Attempting reconnection in {:?} (attempt {}/{}, remaining: {})",
                delay,
                backoff.attempts,
                backoff.config.max_attempts,
                backoff.remaining_attempts()
            );

            sleep(delay).await;

            if self.shutdown.is_cancelled() {
                self.state.transition_to(ConnectionStatus::Shutdown);
                return Ok(());
            }

            match timeout(self.config.reconnect_timeout, self.ws.reconnect()).await {
                Ok(Ok(_)) => {
                    let reconnect_duration = reconnect_start.elapsed();
                    info!(
                        "WebSocket reconnected successfully in {:?}",
                        reconnect_duration
                    );

                    self.state.transition_to(ConnectionStatus::Connected);
                    self.state.mark_successful_operation();
                    self.stats.record_successful_reconnect(reconnect_duration);
                    backoff.reset();

                    // Restart reader task and process queued commands
                    self.start_reader_task();
                    self.process_queued_commands().await;
                    return Ok(());
                }
                Ok(Err(e)) => {
                    warn!("Reconnection attempt {} failed: {}", backoff.attempts, e);
                }
                Err(_) => {
                    warn!(
                        "Reconnection attempt {} timed out after {:?}",
                        backoff.attempts, self.config.reconnect_timeout
                    );
                }
            }
        }

        error!(
            "Reconnection backoff exhausted after {} attempts",
            backoff.config.max_attempts
        );
        self.state.transition_to(ConnectionStatus::Shutdown);
        Err(Error::Internal(
            "Reconnection failed after maximum attempts".into(),
        ))
    }

    async fn stop_reader_task(&mut self) {
        if let Some(handle) = self.reader_handle.take() {
            handle.abort();
            let _ = timeout(Duration::from_secs(2), handle).await;
        }
    }

    #[instrument(skip(self))]
    async fn process_queued_commands(&mut self) {
        let commands = self.command_queue.drain();
        if !commands.is_empty() {
            info!("Processing {} queued commands", commands.len());

            let mut successful = 0;
            let mut failed = 0;

            for cmd in commands {
                match timeout(self.config.command_timeout, self.process_command(cmd)).await {
                    Ok(Ok(_)) => {
                        successful += 1;
                        self.state.mark_successful_operation();
                    }
                    Ok(Err(e)) => {
                        failed += 1;
                        error!("Failed to process queued command: {}", e);
                    }
                    Err(_) => {
                        failed += 1;
                        error!("Queued command timed out");
                    }
                }
            }

            info!(
                "Processed queued commands: {} successful, {} failed",
                successful, failed
            );
        }
    }

    /// Send heartbeat ping (separate from health check)
    async fn send_heartbeat(&self) -> Result<()> {
        let heartbeat_timeout = Duration::from_secs(10);
        match timeout(heartbeat_timeout, self.ws.try_ping()).await {
            Ok(Ok(_)) => {
                debug!("Heartbeat ping successful");
                Ok(())
            }
            Ok(Err(e)) => {
                warn!("Heartbeat ping failed: {}", e);
                Err(Error::Internal(format!("Heartbeat failed: {e}").into()))
            }
            Err(_) => {
                warn!("Heartbeat ping timed out after {:?}", heartbeat_timeout);
                Err(Error::Internal("Heartbeat timeout".into()))
            }
        }
    }

    fn classify_error(&self, error: &Error) -> ErrorSeverity {
        use Error::*;

        match error {
            WebSocket(_) => ErrorSeverity::Recoverable,
            TradingView {
                source: TradingViewError::ProtocolError | TradingViewError::CriticalError,
            } => ErrorSeverity::Recoverable,
            JsonParse(_) | UrlParse(_) => ErrorSeverity::CommandOnly,
            Internal(msg) => {
                let msg_lower = msg.to_lowercase();
                if msg_lower.contains("timeout")
                    || msg_lower.contains("connection")
                    || msg_lower.contains("network")
                {
                    ErrorSeverity::Recoverable
                } else if msg_lower.contains("shutdown") || msg_lower.contains("cancelled") {
                    ErrorSeverity::Fatal
                } else {
                    ErrorSeverity::CommandOnly
                }
            }
            _ => ErrorSeverity::CommandOnly,
        }
    }

    fn log_stats(&mut self) {
        self.stats.total_uptime = self.start_time.elapsed();
        info!(
            "Connection Stats: {:?}, Success Rate: {:.2}%, Uptime: {:?}",
            self.stats,
            self.stats.success_rate() * 100.0,
            self.stats.total_uptime
        );
    }

    #[instrument(skip(self))]
    async fn cleanup(&mut self) {
        info!("Cleaning up CommandRunner");

        // Cancel shutdown token to stop all tasks
        self.shutdown.cancel();

        // Stop reader task
        self.stop_reader_task().await;

        let remaining_commands = self.command_queue.total_len();
        if remaining_commands > 0 {
            warn!(
                "Dropping {} queued commands during cleanup",
                remaining_commands
            );
            self.command_queue.clear();
        }

        // Cleanup WebSocket with timeout
        if let Err(e) = timeout(Duration::from_secs(5), self.ws.delete()).await {
            warn!("WebSocket cleanup timeout: {:?}", e);
        }
    }

    // Public API methods
    pub fn shutdown_token(&self) -> CancellationToken {
        self.shutdown.clone()
    }

    pub fn stats(&self) -> &ConnectionStats {
        &self.stats
    }

    pub fn state(&self) -> &ConnectionState {
        &self.state
    }

    pub fn config(&self) -> &CommandRunnerConfig {
        &self.config
    }
}
