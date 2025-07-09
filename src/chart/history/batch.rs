use crate::{
    DataPoint, DataServer, Error, Interval, MarketSymbol, OHLCV as _, Result, SymbolInfo,
    chart::ChartOptions,
    error::TradingViewError,
    history::resolve_auth_token,
    live::handler::{
        command::CommandRunner,
        message::{Command, TradingViewResponse},
        types::{CommandTx, DataRx, DataTx},
    },
    options::Range,
    websocket::{SeriesInfo, WebSocketClient},
};
use bon::builder;
use dashmap::DashMap;
use serde_json::Value;
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::{
    spawn,
    sync::{Mutex, mpsc, oneshot},
    time::{sleep, timeout},
};
use tokio_util::sync::CancellationToken;
use ustr::{Ustr, ustr};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchCompletionSignal {
    Success,
    Error(Ustr),
    Timeout,
}

/// Tracks completion state and errors for batch processing
#[derive(Debug, Clone)]
struct BatchTracker {
    remaining: Arc<AtomicUsize>,
    errors: Arc<Mutex<Vec<Ustr>>>,
    completed: Arc<Mutex<Vec<Ustr>>>,
    empty: Arc<Mutex<Vec<Ustr>>>,
    // Track mapping between series ID and symbol
    series_map: Arc<DashMap<Ustr, Ustr>>,
    // Track completed series IDs
    completed_series: Arc<Mutex<Vec<Ustr>>>,
    completion_tx: Arc<Mutex<Option<oneshot::Sender<BatchCompletionSignal>>>>,
    // Track if we've received any data at all
    any_data_received: Arc<AtomicBool>,
    // Track timeout for completion
    last_activity: Arc<Mutex<std::time::Instant>>,
}

impl BatchTracker {
    fn new(count: usize, completion_tx: oneshot::Sender<BatchCompletionSignal>) -> Self {
        Self {
            remaining: Arc::new(AtomicUsize::new(count)),
            errors: Arc::new(Mutex::new(Vec::new())),
            completed: Arc::new(Mutex::new(Vec::new())),
            empty: Arc::new(Mutex::new(Vec::new())),
            series_map: Arc::new(DashMap::new()),
            completed_series: Arc::new(Mutex::new(Vec::new())),
            completion_tx: Arc::new(Mutex::new(Some(completion_tx))),
            any_data_received: Arc::new(AtomicBool::new(false)),
            last_activity: Arc::new(Mutex::new(std::time::Instant::now())),
        }
    }

    async fn update_activity(&self) {
        let mut last_activity = self.last_activity.lock().await;
        *last_activity = std::time::Instant::now();
    }

    async fn register_series(&self, series_id: &str, symbol: &str) {
        self.series_map.insert(ustr(series_id), ustr(symbol));
        self.update_activity().await;
        tracing::debug!("Registered series {} for symbol {}", series_id, symbol);
    }

    fn mark_data_received(&self) {
        self.any_data_received.store(true, Ordering::Relaxed);
    }

    async fn mark_completed(&self, series_id: &str, has_data: bool) -> bool {
        let series_id_ustr = ustr(series_id);
        let mut done_series = self.completed_series.lock().await;
        let mut completed = self.completed.lock().await;

        // Check if already completed
        if done_series.contains(&series_id_ustr) {
            tracing::debug!("Series {} already completed", series_id);
            return self.remaining.load(Ordering::Relaxed) == 0;
        }

        done_series.push(series_id_ustr);

        // Get symbol for this series
        let symbol = self
            .series_map
            .get(&series_id_ustr)
            .map(|entry| *entry)
            .unwrap_or_else(|| ustr(&format!("unknown_{series_id}")));

        let remaining = self.remaining.fetch_sub(1, Ordering::Relaxed) - 1;
        completed.push(symbol);

        if !has_data {
            let mut empty = self.empty.lock().await;
            empty.push(symbol);
            tracing::warn!(
                "Series {} (symbol {}) completed with no data",
                series_id,
                symbol
            );
        }

        self.update_activity().await;

        tracing::debug!(
            "Series {} (symbol {}) completed, remaining: {}",
            series_id,
            symbol,
            remaining
        );

        let is_done = remaining == 0;
        if is_done {
            self.signal_completion(BatchCompletionSignal::Success).await;
        }
        is_done
    }

    // Add method to force completion after timeout
    async fn force_completion(&self, reason: &str) {
        let remaining = self.remaining.load(Ordering::Relaxed);
        if remaining > 0 {
            tracing::warn!(
                "Force completing batch: {} (remaining: {})",
                reason,
                remaining
            );
            // Set remaining to 0 to prevent further completions
            self.remaining.store(0, Ordering::Relaxed);

            let any_data = self.any_data_received.load(Ordering::Relaxed);
            if any_data {
                self.signal_completion(BatchCompletionSignal::Success).await;
            } else {
                self.signal_completion(BatchCompletionSignal::Timeout).await;
            }
        }
    }

    async fn add_error(&self, error: Ustr) {
        let mut errors = self.errors.lock().await;
        errors.push(error);

        let remaining = self.remaining.fetch_sub(1, Ordering::Relaxed) - 1;
        self.update_activity().await;

        tracing::error!("Error: {}, remaining: {}", error, remaining);

        if remaining == 0 {
            self.signal_completion(BatchCompletionSignal::Error(error))
                .await;
        }
    }

    async fn signal_completion(&self, signal: BatchCompletionSignal) {
        if let Some(sender) = self.completion_tx.lock().await.take() {
            if let Err(e) = sender.send(signal) {
                tracing::error!("Failed to send batch completion signal: {:?}", e);
            }
        }
    }

    async fn check_timeout(&self, timeout_secs: u64) -> bool {
        let last_activity = self.last_activity.lock().await;
        let elapsed = last_activity.elapsed();

        if elapsed > Duration::from_secs(timeout_secs) {
            drop(last_activity); // Release lock before calling force_completion

            let any_data = self.any_data_received.load(Ordering::Relaxed);
            if any_data {
                self.force_completion(&format!(
                    "timeout after {}s with partial data",
                    elapsed.as_secs()
                ))
                .await;
            } else {
                self.force_completion(&format!(
                    "timeout after {}s with no data",
                    elapsed.as_secs()
                ))
                .await;
            }
            return true;
        }
        false
    }

    fn is_done(&self) -> bool {
        self.remaining.load(Ordering::Relaxed) == 0
    }

    async fn summary(&self) -> (Vec<Ustr>, Vec<Ustr>, Vec<Ustr>) {
        let errors = self.errors.lock().await.clone();
        let completed = self.completed.lock().await.clone();
        let empty = self.empty.lock().await.clone();
        (errors, completed, empty)
    }
}

#[derive(Debug, Clone)]
struct BatchDataCollector {
    pub data: Arc<DashMap<Ustr, Vec<DataPoint>>>,
    pub symbol_info: Arc<DashMap<Ustr, SymbolInfo>>,
    pub series_info: Arc<DashMap<Ustr, SeriesInfo>>,
    pub data_received: Arc<DashMap<Ustr, bool>>, // Track which series received data
}

impl BatchDataCollector {
    fn new() -> Self {
        Self {
            data: Arc::new(DashMap::new()),
            symbol_info: Arc::new(DashMap::new()),
            series_info: Arc::new(DashMap::new()),
            data_received: Arc::new(DashMap::new()),
        }
    }
}

/// Extract series ID from completion message (cs_* pattern)
fn extract_series_id(msg: &[Value]) -> Option<Ustr> {
    if msg.is_empty() {
        tracing::warn!("Empty completion message");
        return None;
    }

    // Look for series ID in message
    for value in msg {
        if let Some(s) = value.as_str() {
            if s.starts_with("cs_") {
                return Some(ustr(s));
            }
        }
    }

    tracing::warn!("No series ID found in: {:?}", msg);
    None
}

/// Set up and initialize WebSocket connection
async fn setup_websocket(
    auth_token: &str,
    server: Option<DataServer>,
    data_tx: DataTx,
) -> Result<Arc<WebSocketClient>> {
    let websocket = WebSocketClient::builder()
        .server(server.unwrap_or(DataServer::ProData))
        .auth_token(auth_token)
        .data_tx(data_tx)
        .build()
        .await?;

    Ok(websocket)
}

/// Send initial commands to set up the trading session
async fn setup_initial_session(cmd_tx: &CommandTx) -> Result<()> {
    // Create quote session
    cmd_tx
        .send(Command::CreateQuoteSession)
        .map_err(|_| Error::Internal(ustr("Failed to send create_quote_session command")))?;

    // Set quote fields
    cmd_tx
        .send(Command::SetQuoteFields)
        .map_err(|_| Error::Internal(ustr("Failed to send set_quote_fields command")))?;

    Ok(())
}

/// Send market setup commands in smaller batches with longer delays
async fn setup_markets_in_batches(
    cmd_tx: &CommandTx,
    symbols: &[impl MarketSymbol],
    interval: Interval,
    range: Option<Range>,
    num_bars: Option<u64>,
    batch_size: usize,
) -> Result<()> {
    let mut commands = Vec::new();

    // Prepare all market commands
    for symbol in symbols {
        let options = ChartOptions::builder()
            .symbol(symbol.symbol().into())
            .exchange(symbol.exchange().into())
            .interval(interval)
            .maybe_range(range.map(|r| r.into()))
            .maybe_bar_count(num_bars)
            .replay_mode(false)
            .build();

        commands.push(Command::set_market(options));
    }

    // Use smaller batch size to reduce connection pressure
    let effective_batch_size = batch_size.min(5); // Reduced further

    // Send commands in smaller batches with longer delays
    for (idx, chunk) in commands.chunks(effective_batch_size).enumerate() {
        tracing::debug!("Processing batch {} with {} symbols", idx + 1, chunk.len());

        for (cmd_idx, cmd) in chunk.iter().enumerate() {
            cmd_tx
                .send(cmd.clone())
                .map_err(|_| Error::Internal(ustr("Failed to send set_market command")))?;

            // Small delay between individual commands
            if cmd_idx < chunk.len() - 1 {
                sleep(Duration::from_millis(500)).await;
            }
        }

        // Longer wait between batches to reduce server load
        if idx < commands.len().div_ceil(effective_batch_size) - 1 {
            sleep(Duration::from_secs(5)).await; // Increased delay
            tracing::debug!("Batch {} done, waiting 5s", idx + 1);
        }
    }

    tracing::debug!("All {} markets set", commands.len());
    Ok(())
}

/// Process incoming TradingViewResponse messages for batch processing
async fn process_batch_data_events(
    mut data_rx: DataRx,
    collector: BatchDataCollector,
    tracker: BatchTracker,
) {
    // Start timeout checker task with shorter intervals
    let timeout_tracker = tracker.clone();
    let timeout_task = spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        loop {
            interval.tick().await;
            if timeout_tracker.check_timeout(60).await {
                // Reduced timeout to 60s
                break;
            }
            if timeout_tracker.is_done() {
                break;
            }
        }
    });

    let mut message_count = 0;
    while let Some(response) = data_rx.recv().await {
        message_count += 1;
        tracing::debug!(
            "Received batch data response #{}: {:?}",
            message_count,
            response
        );

        match response {
            TradingViewResponse::ChartData(series_info, data_points) => {
                handle_batch_chart_data(&collector, &tracker, series_info, data_points).await;
            }
            TradingViewResponse::SymbolInfo(symbol_info) => {
                handle_batch_symbol_info(&collector, symbol_info).await;
                tracker.update_activity().await;
            }
            TradingViewResponse::SeriesCompleted(message) => {
                handle_batch_series_completed(&collector, &tracker, message).await;
            }
            TradingViewResponse::Error(error, message) => {
                handle_batch_error(&tracker, error, message).await;
                // Don't break on errors, continue processing
            }
            _ => {
                tracing::debug!("Received unhandled response type: {:?}", response);
                tracker.update_activity().await;
            }
        }

        // Check if we're done
        if tracker.is_done() {
            tracing::debug!("All series completed, stopping data receiver");
            break;
        }
    }

    timeout_task.abort();
    tracing::debug!(
        "Batch data receiver task completed after {} messages",
        message_count
    );
}

fn is_critical_error(error: &Error) -> bool {
    matches!(
        error,
        Error::TradingView {
            source: TradingViewError::CriticalError
        } | Error::Internal(_) if error.to_string().contains("authentication")
    )
}

async fn handle_batch_chart_data(
    collector: &BatchDataCollector,
    tracker: &BatchTracker,
    series_info: SeriesInfo,
    data_points: Vec<DataPoint>,
) {
    let symbol = ustr(&format!(
        "{}:{}",
        series_info.options.exchange, series_info.options.symbol
    ));
    let series_id = ustr(series_info.chart_session.as_ref());

    tracing::debug!(
        "Received {} points for series {} (symbol {})",
        data_points.len(),
        series_id,
        symbol
    );

    // Mark that we've received data
    tracker.mark_data_received();

    // Register series to symbol mapping
    tracker
        .register_series(series_id.as_ref(), symbol.as_ref())
        .await;

    // Store series info
    collector.series_info.insert(symbol, series_info);

    // Store data points
    collector
        .data
        .entry(symbol)
        .or_default()
        .extend(data_points.iter().cloned());

    // Track data received
    collector
        .data_received
        .insert(series_id, !data_points.is_empty());

    if data_points.is_empty() {
        tracing::warn!("Empty data for series {} (symbol {})", series_id, symbol);
    }
}

async fn handle_batch_symbol_info(collector: &BatchDataCollector, symbol_info: SymbolInfo) {
    let symbol = symbol_info.id;
    tracing::debug!("Received symbol info for {}: {:?}", symbol, symbol_info);
    collector.symbol_info.insert(symbol, symbol_info);
}

async fn handle_batch_series_completed(
    collector: &BatchDataCollector,
    tracker: &BatchTracker,
    message: Vec<Value>,
) {
    tracing::debug!("Series completed: {:?}", message);

    // Try multiple strategies to extract series ID
    let series_id = extract_series_id(&message)
        .or_else(|| {
            // Fallback: look for any string that might be a series ID
            for value in &message {
                if let Some(s) = value.as_str() {
                    if s.contains("cs_") || s.starts_with("series_") {
                        return Some(ustr(s));
                    }
                }
            }
            None
        })
        .unwrap_or_else(|| {
            // Last resort: generate a unique ID based on message hash
            let msg_str = format!("{message:?}");
            // let hash = std::collections::hash_map::DefaultHasher::new();
            ustr(&format!("fallback_{}", msg_str.len()))
        });

    tracing::debug!("Extracted series ID: {}", series_id);

    // Check if series has data
    let has_data = collector
        .data_received
        .get(&series_id)
        .map(|v| *v)
        .unwrap_or(false);

    // Mark as completed and check if all done
    tracker.mark_completed(series_id.as_ref(), has_data).await;
}

async fn handle_batch_error(tracker: &BatchTracker, error: Error, message: Vec<Value>) {
    tracing::error!("WebSocket error: {:?} - {:?}", error, message);

    // Don't treat all errors as critical - just update activity
    tracker.update_activity().await;

    // Only add to error count for truly critical errors
    if is_critical_error(&error) {
        let error_msg = ustr(&format!("Critical WebSocket error: {error:?}"));
        tracker.add_error(error_msg).await;
    }
}

/// Cleanup background tasks
async fn cleanup_batch_tasks(
    shutdown_token: CancellationToken,
    runner_task: tokio::task::JoinHandle<()>,
    data_task: tokio::task::JoinHandle<()>,
) {
    // Signal shutdown
    shutdown_token.cancel();

    // Wait for tasks to complete with timeout
    let cleanup_timeout = Duration::from_secs(5);

    if let Err(e) = timeout(cleanup_timeout, runner_task).await {
        tracing::debug!("Command runner cleanup timeout: {:?}", e);
    }

    if let Err(e) = timeout(cleanup_timeout, data_task).await {
        tracing::debug!("Data receiver cleanup timeout: {:?}", e);
    }

    tracing::debug!("Batch cleanup completed");
}

/// Retrieve historical data for multiple symbols in batches
#[builder]
pub async fn retrieve(
    auth_token: Option<&str>,
    symbols: &[impl MarketSymbol],
    interval: Interval,
    range: Option<Range>,
    server: Option<DataServer>,
    num_bars: Option<u64>,
    #[builder(default = 10)] batch_size: usize, // Reduced default batch size
    #[builder(default = Duration::from_secs(600))] timeout_duration: Duration, // Increased timeout
) -> Result<HashMap<String, (SymbolInfo, Vec<DataPoint>)>> {
    if symbols.is_empty() {
        return Err(Error::Internal(ustr(
            "No symbols provided for batch retrieval",
        )));
    }

    let auth_token = resolve_auth_token(auth_token)?;
    let count = symbols.len();

    tracing::info!(
        "Starting batch retrieval for {} symbols with batch size {}",
        count,
        batch_size
    );

    // Create completion channel
    let (completion_tx, completion_rx) = oneshot::channel::<BatchCompletionSignal>();
    let tracker = BatchTracker::new(count, completion_tx);
    let collector = BatchDataCollector::new();

    // Create data response channel
    let (data_tx, data_rx) = mpsc::unbounded_channel();

    // Create command channel for sending commands to the runner
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();

    // Initialize WebSocket client
    let websocket = setup_websocket(&auth_token, server, data_tx).await?;
    let websocket_shared = Arc::new(websocket);

    // Create and start command runner
    let command_runner = CommandRunner::new(cmd_rx, Arc::clone(&websocket_shared));
    let shutdown_token = command_runner.shutdown_token();
    let runner_task = spawn(async move {
        if let Err(e) = command_runner.run().await {
            tracing::error!("Command runner failed: {}", e);
        }
    });

    // Start data receiver task
    let data_task = spawn(process_batch_data_events(
        data_rx,
        collector.clone(),
        tracker.clone(),
    ));

    // Send initial commands to set up the session
    if let Err(e) = setup_initial_session(&cmd_tx).await {
        cleanup_batch_tasks(shutdown_token, runner_task, data_task).await;
        return Err(e);
    }

    // Set up markets in batches
    if let Err(e) =
        setup_markets_in_batches(&cmd_tx, symbols, interval, range, num_bars, batch_size).await
    {
        cleanup_batch_tasks(shutdown_token, runner_task, data_task).await;
        return Err(e);
    }

    // Wait for completion with timeout
    let result = timeout(timeout_duration, completion_rx).await;

    // Cleanup
    cleanup_batch_tasks(shutdown_token, runner_task, data_task).await;

    match result {
        Ok(Ok(BatchCompletionSignal::Success)) => {
            let mut final_results = HashMap::new();

            // Process collected data
            for entry in collector.data.iter() {
                let (symbol, data_points) = entry.pair();
                let symbol_key = symbol.to_string();

                // Get symbol info
                let symbol_info = collector
                    .symbol_info
                    .get(symbol)
                    .map(|info| info.clone())
                    .unwrap_or_else(|| {
                        // Create default symbol info if not found
                        SymbolInfo {
                            id: *symbol,
                            ..Default::default()
                        }
                    });

                // Clean and sort data points
                let mut points = data_points.clone();
                points.dedup_by_key(|point| point.timestamp());
                points.sort_by_key(|a| a.timestamp());

                final_results.insert(symbol_key, (symbol_info, points));
            }

            let (errors, completed, empty) = tracker.summary().await;

            tracing::info!(
                "Batch completed - Retrieved: {}, Completed: {}, Empty: {}, Errors: {}",
                final_results.len(),
                completed.len(),
                empty.len(),
                errors.len()
            );

            if !errors.is_empty() {
                tracing::warn!("Errors occurred: {:?}", errors);
            }

            if !empty.is_empty() {
                tracing::warn!("Empty symbols: {:?}", empty);
            }

            Ok(final_results)
        }
        Ok(Ok(BatchCompletionSignal::Error(error))) => Err(Error::Internal(error)),
        Ok(Ok(BatchCompletionSignal::Timeout)) => {
            Err(Error::Timeout(ustr("Batch collection timed out")))
        }
        Ok(Err(_)) => Err(Error::Internal(ustr(
            "Batch completion channel closed unexpectedly",
        ))),
        Err(_) => Err(Error::Timeout(ustr(
            "Batch collection timed out after specified duration",
        ))),
    }
}
