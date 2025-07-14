use crate::{
    DataPoint, DataServer, Error, Interval, MarketSymbol, OHLCV as _, Result, SymbolInfo,
    chart::ChartOptions,
    error::TradingViewError,
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

#[derive(Debug, Clone)]
struct BatchTracker {
    total_symbols: usize,
    completed_symbols: Arc<DashMap<Ustr, bool>>,
    failed_symbols: Arc<DashMap<Ustr, bool>>,
    is_complete: Arc<AtomicBool>,
    completion_tx: Arc<Mutex<Option<oneshot::Sender<BatchCompletionSignal>>>>,
    error_count: Arc<AtomicUsize>,

    // Series tracking
    series_to_symbol: Arc<DashMap<Ustr, Ustr>>,
    symbol_by_index: Arc<DashMap<usize, Ustr>>,
}

impl BatchTracker {
    fn new(symbol_count: usize, completion_tx: oneshot::Sender<BatchCompletionSignal>) -> Self {
        Self {
            total_symbols: symbol_count,
            completed_symbols: Arc::new(DashMap::with_capacity(symbol_count)),
            failed_symbols: Arc::new(DashMap::with_capacity(symbol_count)),
            is_complete: Arc::new(AtomicBool::new(false)),
            completion_tx: Arc::new(Mutex::new(Some(completion_tx))),
            error_count: Arc::new(AtomicUsize::new(0)),
            series_to_symbol: Arc::new(DashMap::with_capacity(symbol_count)),
            symbol_by_index: Arc::new(DashMap::with_capacity(symbol_count)),
        }
    }

    fn register_symbol_by_index(&self, index: usize, symbol: Ustr) {
        self.symbol_by_index.insert(index, symbol);
        tracing::debug!("Registered symbol {} at index {}", symbol, index);
    }

    fn register_series(&self, series_id: Ustr, symbol: Ustr) {
        self.series_to_symbol.insert(series_id, symbol);
        tracing::debug!("Registered series {} for symbol {}", series_id, symbol);
    }

    fn mark_symbol_complete(&self, symbol: Ustr) {
        if self.is_complete.load(Ordering::Relaxed) {
            return;
        }

        if self.completed_symbols.insert(symbol, true).is_none() {
            let completed_count = self.completed_symbols.len();
            let failed_count = self.failed_symbols.len();
            let total_processed = completed_count + failed_count;

            tracing::debug!(
                "Symbol {} completed ({}/{}), {} failed",
                symbol,
                completed_count,
                self.total_symbols,
                failed_count
            );

            // Check for overall completion (completed + failed = total)
            if total_processed >= self.total_symbols {
                self.try_complete(BatchCompletionSignal::Success);
            }
        }
    }

    fn mark_symbol_failed(&self, symbol: Ustr, reason: &str) {
        if self.is_complete.load(Ordering::Relaxed) {
            return;
        }

        if self.failed_symbols.insert(symbol, true).is_none() {
            let completed_count = self.completed_symbols.len();
            let failed_count = self.failed_symbols.len();
            let total_processed = completed_count + failed_count;

            tracing::warn!(
                "Symbol {} failed: {} ({} completed, {} failed, {}/{})",
                symbol,
                reason,
                completed_count,
                failed_count,
                total_processed,
                self.total_symbols
            );

            // Check for overall completion (completed + failed = total)
            if total_processed >= self.total_symbols {
                self.try_complete(BatchCompletionSignal::Success);
            }
        }
    }

    fn mark_symbol_failed_by_index(&self, symbol_index: usize, reason: &str) {
        if let Some(entry) = self.symbol_by_index.get(&symbol_index) {
            let symbol = *entry.value();
            self.mark_symbol_failed(symbol, reason);
        }
    }

    async fn add_error(&self, error: Ustr) {
        if self.is_complete.load(Ordering::Relaxed) {
            return;
        }

        let error_count = self.error_count.fetch_add(1, Ordering::Relaxed) + 1;
        tracing::error!("Batch error ({}): {}", error_count, error);

        // Signal completion on critical errors or too many errors
        if error_count >= 5 {
            self.try_complete(BatchCompletionSignal::Error(error));
        }
    }

    fn try_complete(&self, signal: BatchCompletionSignal) {
        if self
            .is_complete
            .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
        {
            // First to complete - send signal
            if let Ok(mut sender) = self.completion_tx.try_lock() {
                if let Some(tx) = sender.take() {
                    let _ = tx.send(signal);
                }
            }
        }
    }

    fn is_done(&self) -> bool {
        self.is_complete.load(Ordering::Relaxed)
    }

    fn get_completion_stats(&self) -> (usize, usize, usize, usize) {
        let completed = self.completed_symbols.len();
        let failed = self.failed_symbols.len();
        let errors = self.error_count.load(Ordering::Relaxed);
        (completed, failed, self.total_symbols, errors)
    }
}

#[derive(Debug, Clone)]
struct BatchDataCollector {
    data: Arc<DashMap<Ustr, Vec<DataPoint>>>,
    symbol_info: Arc<DashMap<Ustr, SymbolInfo>>,
    series_info: Arc<DashMap<Ustr, SeriesInfo>>,
}

impl BatchDataCollector {
    fn new(expected_symbols: usize) -> Self {
        Self {
            data: Arc::new(DashMap::with_capacity(expected_symbols)),
            symbol_info: Arc::new(DashMap::with_capacity(expected_symbols)),
            series_info: Arc::new(DashMap::with_capacity(expected_symbols)),
        }
    }

    fn add_data_points(&self, symbol: Ustr, points: Vec<DataPoint>) -> usize {
        let mut entry = self
            .data
            .entry(symbol)
            .or_insert_with(|| Vec::with_capacity(1000));
        entry.extend(points);
        entry.len()
    }

    fn add_symbol_info(&self, symbol_info: SymbolInfo) {
        self.symbol_info.insert(symbol_info.id, symbol_info);
    }

    fn add_series_info(&self, symbol: Ustr, series_info: SeriesInfo) {
        self.series_info.insert(symbol, series_info);
    }
}

// Reuse helper functions from single implementation
fn extract_series_id(message: &[Value]) -> Option<Ustr> {
    message
        .iter()
        .find_map(|v| v.as_str())
        .filter(|s| s.starts_with("cs_"))
        .map(ustr)
}

/// Set up and initialize WebSocket connection (reused from single)
async fn setup_websocket(
    auth_token: Option<&str>,
    server: Option<DataServer>,
    data_tx: DataTx,
) -> Result<Arc<WebSocketClient>> {
    let websocket = WebSocketClient::builder()
        .server(server.unwrap_or(DataServer::ProData))
        .maybe_auth_token(auth_token)
        .data_tx(data_tx)
        .build()
        .await?;

    Ok(websocket)
}

/// Send initial session setup commands
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

/// Send market setup commands in batches
async fn setup_markets_in_batches(
    cmd_tx: &CommandTx,
    symbols: &[impl MarketSymbol],
    interval: Interval,
    range: Option<Range>,
    num_bars: Option<u64>,
    batch_size: usize,
    tracker: &BatchTracker, // Add tracker parameter
) -> Result<()> {
    let mut commands = Vec::with_capacity(symbols.len());

    // Prepare all market commands and register symbols by index
    for (index, symbol) in symbols.iter().enumerate() {
        let symbol_ustr = ustr(&format!("{}:{}", symbol.exchange(), symbol.symbol()));
        tracker.register_symbol_by_index(index, symbol_ustr);

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

    // Send commands in batches with delays
    let effective_batch_size = batch_size.min(10);

    for (idx, chunk) in commands.chunks(effective_batch_size).enumerate() {
        tracing::debug!("Processing batch {} with {} symbols", idx + 1, chunk.len());

        for (cmd_idx, cmd) in chunk.iter().enumerate() {
            cmd_tx
                .send(cmd.clone())
                .map_err(|_| Error::Internal(ustr("Failed to send set_market command")))?;

            // Small delay between commands
            if cmd_idx < chunk.len() - 1 {
                sleep(Duration::from_millis(200)).await;
            }
        }

        // Wait between batches
        if idx < commands.len().div_ceil(effective_batch_size) - 1 {
            sleep(Duration::from_secs(2)).await;
            tracing::debug!("Batch {} done, waiting 2s", idx + 1);
        }
    }

    tracing::debug!("All {} markets set", commands.len());
    Ok(())
}

// Helper function to extract symbol index from message
fn extract_symbol_index(message: &[Value]) -> Option<usize> {
    // Look for patterns like "sds_sym_3" to extract the index
    for value in message {
        if let Some(s) = value.as_str() {
            if s.starts_with("sds_sym_") {
                if let Ok(index) = s.strip_prefix("sds_sym_").unwrap_or("").parse::<usize>() {
                    return Some(index);
                }
            }
        }
    }
    None
}

/// Process batch data events - simplified using single implementation patterns
async fn process_batch_data_events(
    mut data_rx: DataRx,
    collector: BatchDataCollector,
    tracker: BatchTracker,
) {
    let mut message_count = 0;

    while let Some(response) = data_rx.recv().await {
        if tracker.is_done() {
            tracing::debug!("Tracker done, stopping message processing");
            break;
        }

        message_count += 1;

        match response {
            TradingViewResponse::ChartData(series_info, data_points) => {
                handle_chart_data(&collector, &tracker, series_info, data_points).await;
            }
            TradingViewResponse::SymbolInfo(symbol_info) => {
                collector.add_symbol_info(symbol_info);
            }
            TradingViewResponse::SeriesCompleted(message) => {
                handle_series_completed(&tracker, &message).await;
            }
            TradingViewResponse::Error(error, message) => {
                handle_error(&tracker, error, message).await;
            }
            _ => {
                // Handle other response types
                tracing::debug!("Received unhandled response type");
            }
        }

        // Periodic logging with updated stats
        if message_count % 50 == 0 {
            let (completed, failed, total, errors) = tracker.get_completion_stats();
            tracing::debug!(
                "Processed {} messages, {}/{} symbols completed, {} failed, {} errors",
                message_count,
                completed,
                total,
                failed,
                errors
            );
        }
    }

    tracing::debug!(
        "Batch data processing completed after {} messages",
        message_count
    );
}

async fn handle_chart_data(
    collector: &BatchDataCollector,
    tracker: &BatchTracker,
    series_info: SeriesInfo,
    data_points: Vec<DataPoint>,
) {
    if data_points.is_empty() {
        return;
    }

    let symbol = ustr(&format!(
        "{}:{}",
        series_info.options.exchange, series_info.options.symbol
    ));
    let series_id = ustr(&series_info.chart_session);

    // Register series mapping
    tracker.register_series(series_id, symbol);

    // Store series info and data
    collector.add_series_info(symbol, series_info);
    let total_count = collector.add_data_points(symbol, data_points.clone());

    tracing::debug!(
        "Received {} points for {} (total: {})",
        data_points.len(),
        symbol,
        total_count
    );
}

async fn handle_series_completed(tracker: &BatchTracker, message: &[Value]) {
    if let Some(series_id) = extract_series_id(message) {
        if let Some(entry) = tracker.series_to_symbol.get(&series_id) {
            let symbol = *entry.value();
            tracing::debug!("Series {} completed for symbol {}", series_id, symbol);
            tracker.mark_symbol_complete(symbol);
        }
    }
}

async fn handle_error(tracker: &BatchTracker, error: Error, message: Vec<Value>) {
    tracing::error!("WebSocket error: {:?} - {:?}", error, message);

    // Handle SymbolError specifically - these occur before series registration
    if matches!(
        error,
        Error::TradingView {
            source: TradingViewError::SymbolError
        }
    ) {
        if let Some(symbol_index) = extract_symbol_index(&message) {
            tracker.mark_symbol_failed_by_index(symbol_index, "Symbol resolution failed");
            return;
        }
    }

    // Extract series ID and symbol from error message to mark as failed
    if let Some(series_id) = extract_series_id(&message) {
        if let Some(entry) = tracker.series_to_symbol.get(&series_id) {
            let symbol = *entry.value();
            let error_reason = extract_error_reason(&message);
            tracker.mark_symbol_failed(symbol, &error_reason);
            return; // Don't count as general error if we can attribute it to a specific symbol
        }
    }

    // For unattributed errors, use the original logic
    if is_critical_error(&error) {
        let error_msg = ustr(&format!("Critical error: {error:?}"));
        tracker.add_error(error_msg).await;
    }
}

// Helper function to extract error reason from message
fn extract_error_reason(message: &[Value]) -> String {
    // Look for error description in the message
    for value in message {
        if let Some(s) = value.as_str() {
            if s.contains("invalid symbol") || s.contains("resolve error") || s.contains("error") {
                return s.to_string();
            }
        }
    }
    "Unknown error".to_string()
}

fn is_critical_error(error: &Error) -> bool {
    matches!(
        error,
        Error::TradingView {
            source: TradingViewError::CriticalError
        } | Error::WebSocket(_)
    ) || matches!(error, Error::Internal(_)) && error.to_string().contains("authentication")
}

/// Cleanup background tasks (reused from single)
async fn cleanup_batch_tasks(
    shutdown_token: CancellationToken,
    runner_task: tokio::task::JoinHandle<()>,
    data_task: tokio::task::JoinHandle<()>,
) {
    // Signal shutdown
    shutdown_token.cancel();

    // Wait for tasks to complete with timeout
    let cleanup_timeout = Duration::from_secs(2);

    if let Err(e) = timeout(cleanup_timeout, runner_task).await {
        tracing::debug!("Command runner cleanup timeout: {:?}", e);
    }

    if let Err(e) = timeout(cleanup_timeout, data_task).await {
        tracing::debug!("Data receiver cleanup timeout: {:?}", e);
    }

    tracing::debug!("Batch cleanup completed");
}

/// Main batch retrieval function - simplified
#[builder]
pub async fn retrieve(
    auth_token: Option<&str>,
    symbols: &[impl MarketSymbol],
    interval: Interval,
    range: Option<Range>,
    server: Option<DataServer>,
    num_bars: Option<u64>,
    #[builder(default = 8)] batch_size: usize,
    #[builder(default = Duration::from_secs(180))] timeout_duration: Duration,
) -> Result<HashMap<String, (SymbolInfo, Vec<DataPoint>)>> {
    if symbols.is_empty() {
        return Err(Error::Internal(ustr("No symbols provided")));
    }

    // let auth_token = resolve_auth_token(auth_token)?;
    let symbol_count = symbols.len();

    tracing::info!("Starting batch retrieval for {} symbols", symbol_count);

    // Create components
    let (completion_tx, completion_rx) = oneshot::channel();
    let tracker = BatchTracker::new(symbol_count, completion_tx);
    let collector = BatchDataCollector::new(symbol_count);
    let (data_tx, data_rx) = mpsc::unbounded_channel();
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();

    // Setup WebSocket and command runner
    let websocket = setup_websocket(auth_token, server, data_tx).await?;
    let websocket_shared = Arc::new(websocket);
    let command_runner = CommandRunner::new(cmd_rx, Arc::clone(&websocket_shared));
    let shutdown_token = command_runner.shutdown_token();

    // Start background tasks
    let runner_task = spawn(async move {
        if let Err(e) = command_runner.run().await {
            tracing::error!("Command runner failed: {}", e);
        }
    });

    let data_task = spawn(process_batch_data_events(
        data_rx,
        collector.clone(),
        tracker.clone(),
    ));

    // Setup session and send market commands
    setup_initial_session(&cmd_tx).await?;
    setup_markets_in_batches(
        &cmd_tx, symbols, interval, range, num_bars, batch_size, &tracker,
    )
    .await?;

    // Wait for completion with timeout
    let result = timeout(timeout_duration, completion_rx).await;

    // Cleanup
    cleanup_batch_tasks(shutdown_token, runner_task, data_task).await;

    // Process results
    match result {
        Ok(Ok(BatchCompletionSignal::Success)) => {
            build_final_results(collector, tracker, symbols).await
        }
        Ok(Ok(BatchCompletionSignal::Error(error))) => Err(Error::Internal(error)),
        Ok(Ok(BatchCompletionSignal::Timeout)) => Err(Error::Timeout(ustr("Batch timeout"))),
        Ok(Err(_)) => Err(Error::Internal(ustr("Completion channel closed"))),
        Err(_) => Err(Error::Timeout(ustr("Overall timeout exceeded"))),
    }
}

async fn build_final_results(
    collector: BatchDataCollector,
    tracker: BatchTracker,
    symbols: &[impl MarketSymbol],
) -> Result<HashMap<String, (SymbolInfo, Vec<DataPoint>)>> {
    let mut results = HashMap::with_capacity(collector.data.len());

    // First, mark any symbols that have no data as failed
    for symbol in symbols {
        let symbol_ustr = ustr(&format!("{}:{}", symbol.exchange(), symbol.symbol()));

        // If symbol has no data and wasn't already marked as failed, mark it as failed
        if !collector.data.contains_key(&symbol_ustr)
            && !tracker.failed_symbols.contains_key(&symbol_ustr)
            && !tracker.completed_symbols.contains_key(&symbol_ustr)
        {
            tracker.mark_symbol_failed(symbol_ustr, "No data received");
        }
    }

    for entry in collector.data.iter() {
        let (symbol, data_points) = entry.pair();
        let symbol_str = symbol.to_string();

        // Get symbol info or create default
        let symbol_info = collector
            .symbol_info
            .get(symbol)
            .map(|info| info.clone())
            .unwrap_or_else(|| SymbolInfo {
                id: *symbol,
                ..Default::default()
            });

        // Process data points efficiently (same as single implementation)
        let mut points = data_points.clone();
        if points.len() > 1 {
            points.sort_unstable_by_key(|p| p.timestamp());
            points.dedup_by_key(|p| p.timestamp());
        }

        results.insert(symbol_str, (symbol_info, points));
    }

    let (completed, failed, total, errors) = tracker.get_completion_stats();

    tracing::info!(
        "Batch completed: {}/{} symbols retrieved, {} completed, {} failed, {} errors",
        results.len(),
        total,
        completed,
        failed,
        errors,
    );

    Ok(results)
}
