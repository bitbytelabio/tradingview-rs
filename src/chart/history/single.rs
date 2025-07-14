use crate::{
    DataPoint, DataServer, Error, Interval, MarketSymbol, OHLCV as _, Result, SymbolInfo, Ticker,
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
use serde_json::Value;
use std::{sync::Arc, time::Duration};
use tokio::{
    spawn,
    sync::{Mutex, mpsc, oneshot},
    time::timeout,
};
use tokio_util::sync::CancellationToken;
use ustr::{Ustr, ustr};

#[derive(Debug)]
pub enum CompletionSignal {
    Success,
    Error(String),
    Timeout,
}

#[derive(Debug, Clone)]
struct DataCollector {
    pub data: Arc<Mutex<Vec<DataPoint>>>,
    pub symbol_info: Arc<Mutex<Option<SymbolInfo>>>,
    pub series_info: Arc<Mutex<Option<SeriesInfo>>>,
    pub completion_tx: Arc<Mutex<Option<oneshot::Sender<CompletionSignal>>>>,
    pub error_count: Arc<Mutex<u32>>,
}

impl DataCollector {
    fn new(completion_tx: oneshot::Sender<CompletionSignal>) -> Self {
        Self {
            data: Arc::new(Mutex::new(Vec::new())),
            symbol_info: Arc::new(Mutex::new(None)),
            series_info: Arc::new(Mutex::new(None)),
            completion_tx: Arc::new(Mutex::new(Some(completion_tx))),
            error_count: Arc::new(Mutex::new(0)),
        }
    }

    async fn signal_completion(&self, signal: CompletionSignal) {
        if let Some(sender) = self.completion_tx.lock().await.take() {
            if let Err(e) = sender.send(signal) {
                tracing::error!("Failed to send completion signal: {:?}", e);
            }
        }
    }
}

#[derive(Debug, Clone, Default)]
struct ReplayState {
    pub enabled: bool,
    pub configured: bool,
    pub earliest_ts: Option<i64>,
    pub data_received: bool,
}

/// Fetch historical chart data from TradingView
#[builder]
pub async fn retrieve(
    auth_token: Option<&str>,
    ticker: Option<Ticker>,
    symbol: Option<&str>,
    exchange: Option<&str>,
    interval: Interval,
    range: Option<Range>,
    server: Option<DataServer>,
    num_bars: Option<u64>,
    #[builder(default = false)] with_replay: bool,
    #[builder(default = Duration::from_secs(30))] timeout_duration: Duration,
) -> Result<(SymbolInfo, Vec<DataPoint>)> {
    let range: Option<Ustr> = range.map(|r| r.into());

    let (symbol, exchange) = extract_symbol_exchange(ticker, symbol, exchange)?;

    let options = ChartOptions::builder()
        .symbol(symbol.into())
        .exchange(exchange.into())
        .interval(interval)
        .maybe_range(range)
        .maybe_bar_count(num_bars)
        .replay_mode(false)
        .build();

    // Create completion channel
    let (completion_tx, completion_rx) = oneshot::channel::<CompletionSignal>();
    let data_collector = DataCollector::new(completion_tx);
    let replay_state = Arc::new(Mutex::new(ReplayState {
        enabled: with_replay,
        ..Default::default()
    }));

    // Create data response channel - this will receive TradingViewResponse messages
    let (data_tx, data_rx) = mpsc::unbounded_channel();

    // Create command channel for sending commands to the runner
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();

    // Initialize WebSocket client with the standard data handler
    let websocket = setup_websocket(auth_token, server, data_tx).await?;
    let websocket_shared = Arc::new(websocket);

    // Create and start command runner
    let command_runner = CommandRunner::new(cmd_rx, Arc::clone(&websocket_shared));
    let shutdown_token = command_runner.shutdown_token();
    let runner_task = spawn(async move {
        if let Err(e) = command_runner.run().await {
            tracing::error!("Command runner failed: {}", e);
        }
    });

    // Start data receiver task that processes TradingViewResponse messages
    let data_task = spawn(process_data_events(
        data_rx,
        data_collector.clone(),
        Arc::clone(&replay_state),
        cmd_tx.clone(),
        options,
    ));

    // Send initial commands to set up the session
    if let Err(e) = setup_initial_session(&cmd_tx, &options).await {
        cleanup_tasks(shutdown_token, runner_task, data_task).await;
        return Err(e);
    }

    // Wait for completion with timeout
    let result = timeout(timeout_duration, completion_rx).await;

    // Cleanup
    cleanup_tasks(shutdown_token, runner_task, data_task).await;

    match result {
        Ok(Ok(CompletionSignal::Success)) => {
            let data = data_collector.data.lock().await;
            let symbol_info = data_collector.symbol_info.lock().await;

            let symbol_info = symbol_info
                .as_ref()
                .ok_or_else(|| Error::Internal(ustr("No symbol info received")))?
                .clone();

            let mut data_points = data.clone();
            data_points.dedup_by_key(|point| point.timestamp());
            data_points.sort_by_key(|a| a.timestamp());

            tracing::debug!(
                "Data collection completed with {} points",
                data_points.len()
            );
            Ok((symbol_info, data_points))
        }
        Ok(Ok(CompletionSignal::Error(error))) => Err(Error::Internal(ustr(&error))),
        Ok(Ok(CompletionSignal::Timeout)) => Err(Error::Timeout(ustr("Data collection timed out"))),
        Ok(Err(_)) => Err(Error::Internal(ustr(
            "Completion channel closed unexpectedly",
        ))),
        Err(_) => Err(Error::Timeout(ustr(
            "Data collection timed out after specified duration",
        ))),
    }
}

fn extract_symbol_exchange(
    ticker: Option<Ticker>,
    symbol: Option<&str>,
    exchange: Option<&str>,
) -> Result<(String, String)> {
    if let Some(ticker) = ticker {
        Ok((ticker.symbol().to_string(), ticker.exchange().to_string()))
    } else if let Some(symbol) = symbol {
        if let Some(exchange) = exchange {
            Ok((symbol.to_string(), exchange.to_string()))
        } else {
            Err(Error::TradingView {
                source: TradingViewError::MissingExchange,
            })
        }
    } else {
        Err(Error::TradingView {
            source: TradingViewError::MissingSymbol,
        })
    }
}

/// Set up and initialize WebSocket connection with standard data handler
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

/// Send initial commands to set up the trading session
async fn setup_initial_session(cmd_tx: &CommandTx, options: &ChartOptions) -> Result<()> {
    // Set market configuration
    cmd_tx
        .send(Command::set_market(*options))
        .map_err(|_| Error::Internal(ustr("Failed to send set_market command")))?;

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

/// Process incoming TradingViewResponse messages from data_rx
async fn process_data_events(
    mut data_rx: DataRx,
    collector: DataCollector,
    replay_state: Arc<Mutex<ReplayState>>,
    cmd_tx: CommandTx,
    options: ChartOptions,
) {
    while let Some(response) = data_rx.recv().await {
        tracing::debug!("Received data response: {:?}", response);

        match response {
            TradingViewResponse::ChartData(series_info, data_points) => {
                handle_chart_data(
                    &collector,
                    &replay_state,
                    &cmd_tx,
                    &options,
                    series_info,
                    data_points,
                )
                .await;
            }
            TradingViewResponse::SymbolInfo(symbol_info) => {
                handle_symbol_info(&collector, symbol_info).await;
            }
            TradingViewResponse::SeriesCompleted(message) => {
                handle_series_completed(&collector, &replay_state, message).await;
            }
            TradingViewResponse::Error(error, message) => {
                handle_error(&collector, error, message).await;
            }
            _ => {
                // Handle other response types as needed
                tracing::debug!("Received unhandled response type: {:?}", response);
            }
        }
    }
    tracing::debug!("Data receiver task completed");
}

async fn handle_chart_data(
    collector: &DataCollector,
    replay_state: &Arc<Mutex<ReplayState>>,
    cmd_tx: &CommandTx,
    options: &ChartOptions,
    series_info: SeriesInfo,
    data_points: Vec<DataPoint>,
) {
    tracing::debug!("Received data batch with {} points", data_points.len());

    // Store series info
    *collector.series_info.lock().await = Some(series_info);

    // Store data points
    collector
        .data
        .lock()
        .await
        .extend(data_points.iter().cloned());

    // Update replay state
    {
        let mut state = replay_state.lock().await;
        state.data_received = true;
        if state.enabled && !data_points.is_empty() {
            let earliest = data_points.iter().map(|p| p.timestamp()).min().unwrap_or(0);
            state.earliest_ts = Some(
                state
                    .earliest_ts
                    .map(|existing| existing.min(earliest))
                    .unwrap_or(earliest),
            );
        }
    }

    // Handle replay setup if needed
    if let Err(e) = handle_replay_setup(replay_state, cmd_tx, options, &data_points).await {
        tracing::error!("Failed to setup replay: {}", e);
    }
}

async fn handle_symbol_info(collector: &DataCollector, symbol_info: SymbolInfo) {
    tracing::debug!("Received symbol info: {:?}", symbol_info);
    *collector.symbol_info.lock().await = Some(symbol_info);
}

async fn handle_series_completed(
    collector: &DataCollector,
    replay_state: &Arc<Mutex<ReplayState>>,
    message: Vec<Value>,
) {
    tracing::debug!("Series completed with message: {:?}", message);

    let msg_json = serde_json::to_string(&message)
        .unwrap_or_else(|_| "Failed to serialize message".to_string());

    let state = replay_state.lock().await;
    let should_complete = if state.enabled {
        msg_json.contains("replay") && msg_json.contains("data_completed")
    } else {
        true
    };

    if should_complete {
        collector.signal_completion(CompletionSignal::Success).await;
    }
}

async fn handle_error(collector: &DataCollector, error: Error, message: Vec<Value>) {
    tracing::error!("WebSocket error: {:?} - {:?}", error, message);

    let mut error_count = collector.error_count.lock().await;
    *error_count += 1;

    // Signal completion on critical errors or too many errors
    if is_critical_error(&error) || *error_count > 5 {
        let error_msg = format!("Critical error or too many errors: {error:?}");
        collector
            .signal_completion(CompletionSignal::Error(error_msg))
            .await;
    }
}

fn is_critical_error(error: &Error) -> bool {
    matches!(
        error,
        Error::TradingView {
            source: TradingViewError::CriticalError
        } | Error::WebSocket(_)
    )
}

/// Handle replay mode setup
async fn handle_replay_setup(
    replay_state: &Arc<Mutex<ReplayState>>,
    cmd_tx: &CommandTx,
    options: &ChartOptions,
    data_points: &[DataPoint],
) -> Result<()> {
    let mut state = replay_state.lock().await;

    if state.enabled && !state.configured && state.data_received && !data_points.is_empty() {
        tracing::debug!("Setting up replay mode");

        let earliest_ts = state
            .earliest_ts
            .unwrap_or_else(|| data_points.iter().map(|p| p.timestamp()).min().unwrap_or(0));

        let mut replay_options = *options;
        replay_options.replay_mode = true;
        replay_options.replay_from = earliest_ts;

        tracing::debug!(
            "Setting replay mode with earliest timestamp: {}",
            earliest_ts
        );

        // Send command to update market with replay settings
        cmd_tx
            .send(Command::set_market(replay_options))
            .map_err(|_| Error::Internal(ustr("Failed to send replay market command")))?;

        state.configured = true;
        tracing::debug!("Replay mode configured successfully");
    }

    Ok(())
}

/// Cleanup background tasks
async fn cleanup_tasks(
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

    tracing::debug!("Cleanup completed");
}
