use crate::{Error, Result, error::TradingViewError, live::handler::message::Command};
use std::{collections::VecDeque, sync::Arc};
use tokio::{
    select,
    task::JoinHandle,
    time::{Duration, Instant, interval, sleep, timeout},
};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, instrument, warn};
use ustr::ustr;

use crate::{live::handler::types::CommandRx, websocket::WebSocketClient};

/// Connection state tracking with timestamps for better monitoring
#[derive(Debug, Clone, PartialEq, Copy, Eq)]
pub struct ConnectionState {
    pub status: ConnectionStatus,
    pub last_change: Instant,
    pub last_successful_operation: Option<Instant>,
}

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
    normal_queue: VecDeque<Command>,
    priority_queue: VecDeque<Command>,
    max_size: usize,
    dropped_count: u64,
}

impl CommandQueue {
    fn new(max_size: usize) -> Self {
        Self {
            normal_queue: VecDeque::new(),
            priority_queue: VecDeque::new(),
            max_size,
            dropped_count: 0,
        }
    }

    fn enqueue(&mut self, cmd: Command) -> bool {
        let is_priority = Self::is_priority_command(&cmd);

        if is_priority {
            // Handle priority commands
            if self.priority_queue.len() >= self.max_size {
                // Don't drop priority commands if queue is at absolute max
                return false;
            }
            self.priority_queue.push_back(cmd);
        } else {
            // Handle normal commands
            let max_normal_capacity = self.max_size / 2;

            // Drop oldest normal command if queue is full
            if self.normal_queue.len() >= max_normal_capacity {
                if let Some(_dropped) = self.normal_queue.pop_front() {
                    self.dropped_count += 1;
                    warn!(
                        "Dropped normal command due to queue overflow (total dropped: {})",
                        self.dropped_count
                    );
                }
            }

            self.normal_queue.push_back(cmd);
        }

        true
    }

    fn drain(&mut self) -> Vec<Command> {
        // Priority commands first, then normal commands
        let mut commands = Vec::with_capacity(self.len());

        // Drain priority queue first
        while let Some(cmd) = self.priority_queue.pop_front() {
            commands.push(cmd);
        }

        // Then drain normal queue
        while let Some(cmd) = self.normal_queue.pop_front() {
            commands.push(cmd);
        }

        commands
    }

    fn clear(&mut self) {
        self.priority_queue.clear();
        self.normal_queue.clear();
    }

    fn len(&self) -> usize {
        self.priority_queue.len() + self.normal_queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.priority_queue.is_empty() && self.normal_queue.is_empty()
    }

    pub fn capacity_info(&self) -> (usize, usize, usize) {
        let total_capacity = self.max_size;
        let priority_capacity = self.max_size;
        let normal_capacity = self.max_size / 2;
        (total_capacity, priority_capacity, normal_capacity)
    }

    fn is_priority_command(cmd: &Command) -> bool {
        matches!(
            cmd,
            Command::SetAuthToken { .. }
                | Command::Delete
                | Command::Ping
                | Command::CreateQuoteSession
                | Command::CreateChartSession { .. }
        )
    }

    fn stats(&self) -> (usize, usize, u64) {
        (
            self.priority_queue.len(),
            self.normal_queue.len(),
            self.dropped_count,
        )
    }

    /// Get detailed queue statistics
    pub fn detailed_stats(&self) -> CommandQueueStats {
        let (total_cap, priority_cap, normal_cap) = self.capacity_info();
        CommandQueueStats {
            priority_queue_len: self.priority_queue.len(),
            normal_queue_len: self.normal_queue.len(),
            total_len: self.len(),
            priority_queue_capacity: priority_cap,
            normal_queue_capacity: normal_cap,
            total_capacity: total_cap,
            dropped_count: self.dropped_count,
            priority_utilization: if priority_cap > 0 {
                (self.priority_queue.len() as f64 / priority_cap as f64) * 100.0
            } else {
                0.0
            },
            normal_utilization: if normal_cap > 0 {
                (self.normal_queue.len() as f64 / normal_cap as f64) * 100.0
            } else {
                0.0
            },
        }
    }
}

/// Detailed statistics for the command queue
#[derive(Debug, Clone)]
pub struct CommandQueueStats {
    pub priority_queue_len: usize,
    pub normal_queue_len: usize,
    pub total_len: usize,
    pub priority_queue_capacity: usize,
    pub normal_queue_capacity: usize,
    pub total_capacity: usize,
    pub dropped_count: u64,
    pub priority_utilization: f64, // Percentage
    pub normal_utilization: f64,   // Percentage
}

impl std::fmt::Display for CommandQueueStats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Queue Stats: P:{}/{} ({:.1}%), N:{}/{} ({:.1}%), Dropped:{}",
            self.priority_queue_len,
            self.priority_queue_capacity,
            self.priority_utilization,
            self.normal_queue_len,
            self.normal_queue_capacity,
            self.normal_utilization,
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

pub struct CommandRunner {
    rx: CommandRx,
    ws: Arc<WebSocketClient>,
    shutdown: CancellationToken,
    state: ConnectionState,
    command_queue: CommandQueue,
    stats: ConnectionStats,
    reader_handle: Option<JoinHandle<()>>,
    config: CommandRunnerConfig,
    start_time: Instant,
}

impl CommandRunner {
    pub fn new(rx: CommandRx, ws: Arc<WebSocketClient>) -> Self {
        Self::with_config(rx, ws, CommandRunnerConfig::default())
    }

    pub fn with_config(
        rx: CommandRx,
        ws: Arc<WebSocketClient>,
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
                    if let Err(e) = self.check_connection_health().await {
                        warn!("Health check failed: {}", e);
                        self.state.transition_to(ConnectionStatus::Disconnected);
                    }
                },

                _ = hb.tick() => {
                    if self.state.status == ConnectionStatus::Connected {
                        if let Err(e) = self.send_heartbeat().await {
                            warn!("Heartbeat failed: {}", e);
                            self.state.transition_to(ConnectionStatus::Disconnected);
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
        self.log_final_stats();
        Ok(())
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

        // Create a quote session if not already created
        if self.ws.quote_session.read().await.is_empty() {
            info!("Creating initial quote session");
            self.ws.create_quote_session().await?;
            self.ws.set_fields().await?;
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
                if self.command_queue.enqueue(cmd) {
                    let (priority, normal, dropped) = self.command_queue.stats();
                    debug!(
                        "Command queued during reconnection (priority: {}, normal: {}, dropped: {})",
                        priority, normal, dropped
                    );
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
                    self.command_queue.enqueue(cmd);
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
        use Command::*;

        let result = timeout(self.config.command_timeout, async {
            match cmd {
                Ping => self
                    .ws
                    .try_ping()
                    .await
                    .map_err(|e| Error::Internal(ustr(&e.to_string()))),
                Delete => {
                    self.cleanup().await;
                    Ok(())
                }
                SetAuthToken { auth_token } => {
                    self.ws.set_auth_token(&auth_token).await?;
                    Ok(())
                }
                SetLocals { language, country } => {
                    self.ws.set_locale((&language, &country)).await?;
                    Ok(())
                }
                SetDataQuality { quality } => {
                    self.ws.set_data_quality(&quality).await?;
                    Ok(())
                }
                SetTimeZone { session, timezone } => {
                    self.ws.set_timezone(&session, timezone).await?;
                    Ok(())
                }
                CreateQuoteSession => {
                    self.ws.create_quote_session().await?;
                    Ok(())
                }
                DeleteQuoteSession => {
                    self.ws.delete_quote_session().await?;
                    Ok(())
                }
                SetQuoteFields => {
                    self.ws.set_fields().await?;
                    Ok(())
                }
                FastSymbols { symbols } => {
                    let symbols: Vec<_> = symbols.into_iter().map(|s| s.as_str()).collect();
                    self.ws.fast_symbols(&symbols).await?;
                    Ok(())
                }
                AddSymbols { symbols } => {
                    let symbols: Vec<_> = symbols.into_iter().map(|s| s.as_str()).collect();
                    self.ws.add_symbols(&symbols).await?;
                    Ok(())
                }
                RemoveSymbols { symbols } => {
                    let symbols: Vec<_> = symbols.into_iter().map(|s| s.as_str()).collect();
                    self.ws.remove_symbols(&symbols).await?;
                    Ok(())
                }
                CreateChartSession { session } => {
                    self.ws.create_chart_session(&session).await?;
                    Ok(())
                }
                DeleteChartSession { session } => {
                    self.ws.delete_chart_session(&session).await?;
                    Ok(())
                }
                RequestMoreData {
                    session,
                    series_id,
                    bar_count,
                } => {
                    self.ws
                        .request_more_data(&session, &series_id, bar_count)
                        .await?;
                    Ok(())
                }
                RequestMoreTickMarks {
                    session,
                    series_id,
                    bar_count,
                } => {
                    self.ws
                        .request_more_tickmarks(&session, &series_id, bar_count)
                        .await?;
                    Ok(())
                }
                CreateStudy {
                    session,
                    study_id,
                    series_id,
                    indicator,
                } => {
                    self.ws
                        .create_study(&session, &study_id, &series_id, indicator)
                        .await?;
                    Ok(())
                }
                ModifyStudy {
                    session,
                    study_id,
                    series_id,
                    indicator,
                } => {
                    self.ws
                        .modify_study(&session, &study_id, &series_id, indicator)
                        .await?;
                    Ok(())
                }
                RemoveStudy {
                    session,
                    study_id,
                    series_id,
                } => {
                    let id = format!("{series_id}_{study_id}");
                    self.ws.remove_study(&session, &id).await?;
                    Ok(())
                }
                SetStudy {
                    study_options,
                    session,
                    series_id,
                } => {
                    self.ws
                        .set_study(study_options, &session, &series_id)
                        .await?;
                    Ok(())
                }
                CreateSeries {
                    session,
                    series_id,
                    series_version,
                    series_symbol_id,
                    config,
                } => {
                    self.ws
                        .create_series(
                            &session,
                            &series_id,
                            &series_version,
                            &series_symbol_id,
                            config,
                        )
                        .await?;
                    Ok(())
                }
                ModifySeries {
                    session,
                    series_id,
                    series_version,
                    series_symbol_id,
                    config,
                } => {
                    self.ws
                        .modify_series(
                            &session,
                            &series_id,
                            &series_version,
                            &series_symbol_id,
                            config,
                        )
                        .await?;
                    Ok(())
                }
                RemoveSeries { session, series_id } => {
                    self.ws.remove_series(&session, &series_id).await?;
                    Ok(())
                }
                CreateReplaySession { session } => {
                    self.ws.create_replay_session(&session).await?;
                    Ok(())
                }
                DeleteReplaySession { session } => {
                    self.ws.delete_replay_session(&session).await?;
                    Ok(())
                }
                ResolveSymbol {
                    session,
                    symbol,
                    exchange,
                    opts,
                    replay_session,
                } => {
                    self.ws
                        .resolve_symbol(
                            &session,
                            &symbol,
                            &exchange,
                            opts,
                            replay_session.as_deref(),
                        )
                        .await?;
                    Ok(())
                }
                SetReplayStep {
                    session,
                    series_id,
                    step,
                } => {
                    self.ws.replay_step(&session, &series_id, step).await?;
                    Ok(())
                }
                StartReplay {
                    session,
                    series_id,
                    interval,
                } => {
                    self.ws.replay_start(&session, &series_id, interval).await?;
                    Ok(())
                }
                StopReplay { session, series_id } => {
                    self.ws.replay_stop(&session, &series_id).await?;
                    Ok(())
                }
                ResetReplay {
                    session,
                    series_id,
                    timestamp,
                } => {
                    self.ws
                        .replay_reset(&session, &series_id, timestamp)
                        .await?;
                    Ok(())
                }
                SetReplay {
                    symbol,
                    options,
                    chart_session,
                    symbol_series_id,
                } => {
                    self.ws
                        .set_replay(&symbol, options, &chart_session, &symbol_series_id)
                        .await?;
                    Ok(())
                }
                SetMarket { options } => {
                    self.ws.set_market(options).await?;
                    Ok(())
                }
            }
        })
        .await;

        match result {
            Ok(cmd_result) => cmd_result,
            Err(_) => Err(Error::Internal("Command timeout".into())),
        }
    }

    fn is_critical_command(&self, cmd: &Command) -> bool {
        matches!(
            cmd,
            Command::SetAuthToken { .. }
                | Command::Delete
                | Command::CreateQuoteSession
                | Command::CreateChartSession { .. }
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

    async fn check_connection_health(&self) -> Result<()> {
        // Check if connection has been unhealthy for too long
        if let Some(time_since_success) = self.state.time_since_last_success() {
            if time_since_success > self.config.health_check_timeout {
                return Err(Error::Internal(
                    format!("No successful operations for {time_since_success:?}").into(),
                ));
            }
        }

        if self.ws.is_closed().await {
            return Err(Error::Internal("WebSocket is closed".into()));
        }

        Ok(())
    }

    async fn send_heartbeat(&self) -> Result<()> {
        timeout(Duration::from_secs(5), self.ws.try_ping())
            .await
            .map_err(|_| Error::Internal("Heartbeat timeout".into()))?
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
        let (priority_queued, normal_queued, dropped) = self.command_queue.stats();

        info!(
            "CommandRunner Stats - Uptime: {:?}, Commands: {}/{} ({:.1}% success), Reconnects: {}/{}, Queue: P:{} N:{} D:{}, Avg Latency: {:?}",
            self.stats.total_uptime,
            self.stats.commands_processed,
            self.stats.commands_processed + self.stats.commands_failed,
            self.stats.success_rate() * 100.0,
            self.stats.successful_reconnects,
            self.stats.reconnect_attempts,
            priority_queued,
            normal_queued,
            dropped,
            self.stats.average_command_latency
        );
    }

    fn log_final_stats(&self) {
        info!("CommandRunner final stats: {:?}", self.stats);
    }

    #[instrument(skip(self))]
    async fn cleanup(&mut self) {
        info!("Cleaning up CommandRunner");

        // Cancel shutdown token to stop all tasks
        self.shutdown.cancel();

        // Stop reader task
        self.stop_reader_task().await;

        let remaining_commands = self.command_queue.len();
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

    pub fn queue_stats(&self) -> (usize, usize, u64) {
        self.command_queue.stats()
    }
}
