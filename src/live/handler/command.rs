use crate::{Error, Result, error::TradingViewError, live::handler::message::Command};
use std::sync::Arc;
use tokio::{
    select,
    task::JoinHandle,
    time::{Duration, interval, sleep, timeout},
};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};
use ustr::ustr;

use crate::{live::handler::types::CommandRx, websocket::WebSocketClient};

/// Connection state tracking
#[derive(Debug, Clone, PartialEq, Copy)]
pub enum ConnectionState {
    Connected,
    Disconnected,
    Reconnecting,
    Shutdown,
}

/// Error classification for better handling
#[derive(Debug, Clone, Copy)]
enum ErrorSeverity {
    /// Temporary errors that should trigger reconnection
    Recoverable,
    /// Permanent errors that should stop the runner
    Fatal,
    /// Command-specific errors that don't affect connection
    CommandOnly,
}

/// Exponential backoff strategy for reconnection attempts
struct ExponentialBackoff {
    current_delay: Duration,
    max_delay: Duration,
    max_attempts: usize,
    attempts: usize,
    multiplier: f64,
    jitter: bool,
}

impl ExponentialBackoff {
    fn new() -> Self {
        Self {
            current_delay: Duration::from_millis(1000), // Start with 1 second
            max_delay: Duration::from_secs(60),         // Cap at 60 seconds
            max_attempts: 10,                           // Max 10 attempts
            attempts: 0,
            multiplier: 2.0, // Double each time
            jitter: true,    // Add randomness to prevent thundering herd
        }
    }

    fn next_backoff(&mut self) -> Option<Duration> {
        if self.attempts >= self.max_attempts {
            return None;
        }

        let mut delay = self.current_delay;
        self.attempts += 1;

        // Add jitter to prevent thundering herd problem
        if self.jitter {
            let jitter_range = delay.as_millis() as f64 * 0.1; // 10% jitter
            let jitter = (rand::random::<f64>() - 0.5) * 2.0 * jitter_range;
            delay = Duration::from_millis((delay.as_millis() as f64 + jitter).max(0.0) as u64);
        }

        // Exponential backoff
        self.current_delay = std::cmp::min(
            Duration::from_millis((self.current_delay.as_millis() as f64 * self.multiplier) as u64),
            self.max_delay,
        );

        Some(delay)
    }

    fn reset(&mut self) {
        self.current_delay = Duration::from_millis(1000);
        self.attempts = 0;
    }
}

/// Command queue for handling commands during reconnection
#[derive(Debug)]
struct CommandQueue {
    queue: Vec<Command>,
    max_size: usize,
}

impl CommandQueue {
    fn new() -> Self {
        Self {
            queue: Vec::new(),
            max_size: 100, // Prevent memory leaks
        }
    }

    fn enqueue(&mut self, cmd: Command) -> bool {
        if self.queue.len() >= self.max_size {
            warn!("Command queue full, dropping oldest command");
            self.queue.remove(0);
        }
        self.queue.push(cmd);
        true
    }

    fn drain(&mut self) -> Vec<Command> {
        std::mem::take(&mut self.queue)
    }

    fn clear(&mut self) {
        self.queue.clear();
    }

    fn len(&self) -> usize {
        self.queue.len()
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
}

#[derive(Debug, Default, Clone, Copy)]
pub struct ConnectionStats {
    reconnect_attempts: u64,
    successful_reconnects: u64,
    commands_processed: u64,
    errors_handled: u64,
}

impl CommandRunner {
    pub fn new(rx: CommandRx, ws: Arc<WebSocketClient>) -> Self {
        Self {
            rx,
            ws,
            shutdown: CancellationToken::new(),
            state: ConnectionState::Connected,
            command_queue: CommandQueue::new(),
            stats: ConnectionStats::default(),
            reader_handle: None,
        }
    }

    pub async fn run(mut self) -> Result<()> {
        let mut hb = interval(Duration::from_secs(30));
        let mut backoff = ExponentialBackoff::new();
        let mut health_check = interval(Duration::from_secs(5));

        info!("CommandRunner started");

        // Start the WebSocket reader task
        self.start_reader_task();

        // Send initial authentication if we have a token
        let auth_token = *self.ws.auth_token.read().await;
        if auth_token != "unauthorized_user_token" {
            info!("Sending initial authentication token");
            if let Err(e) = self.ws.set_auth_token(&auth_token).await {
                warn!("Failed to set initial auth token: {}", e);
            }
        }

        // Create a quote session if not already created
        if self.ws.quote_session.read().await.is_empty() {
            info!("Creating initial quote session");
            if let Err(e) = self.ws.create_quote_session().await {
                warn!("Failed to create initial quote session: {}", e);
            }

            if let Err(e) = self.ws.set_fields().await {
                warn!("Failed to set quote fields: {}", e);
            }
        }

        loop {
            select! {
                biased;

                _ = self.shutdown.cancelled() => {
                    info!("Shutdown signal received");
                    self.state = ConnectionState::Shutdown;
                    break;
                },

                cmd = self.rx.recv() => match cmd {
                    Some(cmd) => {
                        debug!("Processing command: {:?}", cmd);
                        if let Err(e) = self.handle_command(cmd, &mut backoff).await {
                            self.stats.errors_handled += 1;
                            match self.classify_error(&e) {
                                ErrorSeverity::Fatal => {
                                    error!("Fatal error, shutting down: {}", e);
                                    break;
                                },
                                ErrorSeverity::Recoverable => {
                                    warn!("Recoverable error, will attempt reconnection: {}", e);
                                    self.state = ConnectionState::Disconnected;
                                },
                                ErrorSeverity::CommandOnly => {
                                    warn!("Command error (continuing): {}", e);
                                },
                            }
                        } else {
                            self.stats.commands_processed += 1;
                            debug!("Command processed successfully");
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
                        self.state = ConnectionState::Disconnected;
                    }
                },

                _ = hb.tick() => {
                    if self.state == ConnectionState::Connected {
                        if let Err(e) = self.send_heartbeat().await {
                            warn!("Heartbeat failed: {}", e);
                            self.state = ConnectionState::Disconnected;
                        }
                    }
                },
            }

            if self.state == ConnectionState::Disconnected {
                if let Err(e) = self.handle_reconnection(&mut backoff).await {
                    error!("Reconnection failed: {}", e);
                    break;
                }
            }
        }

        self.cleanup().await;
        info!("CommandRunner stopped. Stats: {:?}", self.stats);
        Ok(())
    }

    fn start_reader_task(&mut self) {
        if self.reader_handle.is_none() {
            let ws = Arc::clone(&self.ws);
            self.reader_handle = Some(tokio::spawn(async move {
                info!("Starting WebSocket reader task");
                if let Err(e) = ws.subscribe().await {
                    error!("WebSocket reader task failed: {}", e);
                } else {
                    info!("WebSocket reader task completed successfully");
                }
            }));
        }
    }

    async fn handle_command(
        &mut self,
        cmd: Command,
        backoff: &mut ExponentialBackoff,
    ) -> Result<()> {
        match self.state {
            ConnectionState::Connected => self.process_command(cmd).await,
            ConnectionState::Reconnecting => {
                // Queue commands during reconnection
                if self.command_queue.enqueue(cmd) {
                    debug!(
                        "Command queued during reconnection (queue size: {})",
                        self.command_queue.len()
                    );
                }
                Ok(())
            }
            ConnectionState::Disconnected => {
                // Attempt immediate reconnection for critical commands
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
            ConnectionState::Shutdown => {
                warn!("Ignoring command after shutdown");
                Ok(())
            }
        }
    }

    async fn process_command(&mut self, cmd: Command) -> Result<()> {
        use Command::*;

        // Add timeout to prevent hanging
        let result = timeout(Duration::from_secs(10), async {
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
        matches!(cmd, Command::SetAuthToken { .. } | Command::Delete)
    }

    async fn handle_reconnection(&mut self, backoff: &mut ExponentialBackoff) -> Result<()> {
        if self.state == ConnectionState::Shutdown {
            return Ok(());
        }

        self.state = ConnectionState::Reconnecting;
        self.stats.reconnect_attempts += 1;

        // Stop the old reader task
        if let Some(handle) = self.reader_handle.take() {
            handle.abort();
        }

        while let Some(delay) = backoff.next_backoff() {
            info!(
                "Attempting reconnection in {:?} (attempt {}/{})",
                delay, backoff.attempts, backoff.max_attempts
            );

            sleep(delay).await;

            if self.shutdown.is_cancelled() {
                self.state = ConnectionState::Shutdown;
                return Ok(());
            }

            match timeout(Duration::from_secs(30), self.ws.reconnect()).await {
                Ok(Ok(_)) => {
                    info!("WebSocket reconnected successfully");
                    self.state = ConnectionState::Connected;
                    self.stats.successful_reconnects += 1;
                    backoff.reset();

                    // Start new reader task
                    self.start_reader_task();

                    self.process_queued_commands().await;
                    return Ok(());
                }
                Ok(Err(e)) => {
                    warn!("Reconnection attempt {} failed: {}", backoff.attempts, e);
                }
                Err(_) => {
                    warn!("Reconnection attempt {} timed out", backoff.attempts);
                }
            }
        }

        error!(
            "Reconnection backoff exhausted after {} attempts",
            backoff.max_attempts
        );
        self.state = ConnectionState::Shutdown;
        Err(Error::Internal(
            "Reconnection failed after maximum attempts".into(),
        ))
    }

    async fn process_queued_commands(&mut self) {
        let commands = self.command_queue.drain();
        if !commands.is_empty() {
            info!("Processing {} queued commands", commands.len());

            for cmd in commands {
                if let Err(e) = self.process_command(cmd).await {
                    error!("Failed to process queued command: {}", e);
                    // Continue processing other commands
                }
            }
        }
    }

    async fn check_connection_health(&self) -> Result<()> {
        if self.ws.is_closed().await {
            return Err(Error::Internal("WebSocket is closed".into()));
        }
        Ok(())
    }

    async fn send_heartbeat(&self) -> Result<()> {
        self.ws.try_ping().await
    }

    fn classify_error(&self, error: &Error) -> ErrorSeverity {
        use Error::*;

        match error {
            // Network/connection errors are recoverable
            WebSocket(_) => ErrorSeverity::Recoverable,

            // Authentication errors might be recoverable
            TradingView {
                source: TradingViewError::ProtocolError | TradingViewError::CriticalError,
            } => ErrorSeverity::Recoverable,

            // Parse errors are usually command-specific
            JsonParse(_) => ErrorSeverity::CommandOnly,
            UrlParse(_) => ErrorSeverity::CommandOnly,

            // Internal errors depend on context
            Internal(msg) => {
                if msg.contains("timeout") || msg.contains("connection") {
                    ErrorSeverity::Recoverable
                } else {
                    ErrorSeverity::Fatal
                }
            }

            // Default to command-only for unknown errors
            _ => ErrorSeverity::CommandOnly,
        }
    }

    async fn cleanup(&mut self) {
        info!("Cleaning up CommandRunner");

        // Stop reader task
        if let Some(handle) = self.reader_handle.take() {
            handle.abort();
            if (timeout(Duration::from_secs(2), handle).await).is_err() {
                warn!("Reader task did not stop gracefully");
            }
        }

        let remaining_commands = self.command_queue.len();
        if remaining_commands > 0 {
            warn!(
                "Dropping {} queued commands during cleanup",
                remaining_commands
            );
            self.command_queue.clear();
        }

        if let Err(e) = timeout(Duration::from_secs(5), self.ws.delete()).await {
            warn!("WebSocket cleanup timeout: {:?}", e);
        }
    }

    /// Get a handle to request shutdown
    pub fn shutdown_token(&self) -> CancellationToken {
        self.shutdown.clone()
    }

    /// Get connection statistics
    pub fn stats(&self) -> &ConnectionStats {
        &self.stats
    }

    /// Get current connection state
    pub fn state(&self) -> &ConnectionState {
        &self.state
    }
}
