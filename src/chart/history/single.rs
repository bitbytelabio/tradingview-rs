use crate::{
    ChartHistoricalData, DataPoint, Error, Interval, MarketSymbol, MarketTicker, OHLCV as _,
    Result, SymbolInfo,
    chart::ChartOptions,
    error::TradingViewError,
    handler::event::TradingViewEventHandlers,
    history::resolve_auth_token,
    options::Range,
    socket::DataServer,
    websocket::{SeriesInfo, WebSocketClient, WebSocketHandler},
};
use bon::builder;
use serde_json::Value;
use std::{sync::Arc, time::Duration};
use tokio::{
    spawn,
    sync::{Mutex, mpsc, oneshot},
    time::{sleep, timeout},
};

pub type DataSender = Arc<Mutex<mpsc::Sender<(SeriesInfo, Vec<DataPoint>)>>>;
pub type DataReceiver = Arc<Mutex<mpsc::Receiver<(SeriesInfo, Vec<DataPoint>)>>>;
pub type InfoSender = Arc<Mutex<mpsc::Sender<SymbolInfo>>>;
pub type InfoReceiver = Arc<Mutex<mpsc::Receiver<SymbolInfo>>>;
pub type ErrorSender = Arc<Mutex<mpsc::Sender<(Error, Vec<Value>)>>>;
pub type ErrorReceiver = Arc<Mutex<mpsc::Receiver<(Error, Vec<Value>)>>>;

#[derive(Debug)]
pub enum CompletionSignal {
    Success,
    Error(String),
    Timeout,
}

#[derive(Debug, Clone)]
struct Senders {
    pub data: DataSender,
    pub info: InfoSender,
    pub error: ErrorSender,
    pub completion: Arc<Mutex<Option<oneshot::Sender<CompletionSignal>>>>,
}

#[derive(Debug, Clone)]
struct Receivers {
    pub data: DataReceiver,
    pub info: InfoReceiver,
    pub error: ErrorReceiver,
    pub completion: Arc<Mutex<oneshot::Receiver<CompletionSignal>>>,
}

#[derive(Debug, Clone)]
struct Channels {
    pub senders: Senders,
    pub receivers: Receivers,
}

impl Channels {
    pub fn new() -> Self {
        let (data_tx, data_rx) = mpsc::channel::<(SeriesInfo, Vec<DataPoint>)>(2000);
        let (info_tx, info_rx) = mpsc::channel::<SymbolInfo>(2000);
        let (error_tx, error_rx) = mpsc::channel::<(Error, Vec<Value>)>(2000);
        let (completion_tx, completion_rx) = oneshot::channel::<CompletionSignal>();

        Self {
            senders: Senders {
                data: Arc::new(Mutex::new(data_tx)),
                info: Arc::new(Mutex::new(info_tx)),
                error: Arc::new(Mutex::new(error_tx)),
                completion: Arc::new(Mutex::new(Some(completion_tx))),
            },
            receivers: Receivers {
                data: Arc::new(Mutex::new(data_rx)),
                info: Arc::new(Mutex::new(info_rx)),
                error: Arc::new(Mutex::new(error_rx)),
                completion: Arc::new(Mutex::new(completion_rx)),
            },
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
    ticker: Option<&MarketTicker>,
    symbol: Option<&str>,
    exchange: Option<&str>,
    interval: Interval,
    range: Option<Range>,
    server: Option<DataServer>,
    num_bars: Option<u64>,
    #[builder(default = false)] with_replay: bool,
    #[builder(default = Duration::from_secs(30))] timeout_duration: Duration,
) -> Result<(SymbolInfo, Vec<DataPoint>)> {
    let auth_token = resolve_auth_token(auth_token)?;
    let range = range.map(String::from);

    let (symbol, exchange) = extract_symbol_exchange(ticker, symbol, exchange)?;

    let options = ChartOptions::builder()
        .symbol(symbol)
        .exchange(exchange)
        .interval(interval)
        .maybe_range(range)
        .maybe_bar_count(num_bars)
        .replay_mode(false) // Start without replay mode
        .build();

    let channels = Channels::new();
    let replay_state = Arc::new(Mutex::new(ReplayState {
        enabled: with_replay,
        ..Default::default()
    }));

    // Create callback handlers
    let callbacks = create_callbacks(channels.senders.clone(), Arc::clone(&replay_state));

    // Initialize and configure WebSocket connection
    let websocket = setup_websocket(&auth_token, server, callbacks, options).await?;
    let websocket_shared = Arc::new(websocket);

    // Start the subscription process in a background task
    let ws_for_sub = Arc::clone(&websocket_shared);
    let sub_task = spawn(async move { ws_for_sub.subscribe().await });

    // Collect and process incoming data with timeout
    let result = timeout(
        timeout_duration,
        collect_data(channels.receivers, &websocket_shared, replay_state),
    )
    .await;

    // Cleanup resources
    cleanup(websocket_shared, sub_task).await;

    match result {
        Ok(Ok(result)) => {
            tracing::debug!(
                "Data collection completed with {} points",
                result.data.len()
            );

            let mut data = result.data;
            data.dedup_by_key(|point| point.timestamp());
            data.sort_by_key(|a| a.timestamp());

            Ok((result.symbol_info, data))
        }
        Ok(Err(error)) => Err(error),
        Err(_) => Err(Error::TimeoutError("Data collection timed out".to_string())),
    }
}

fn extract_symbol_exchange(
    ticker: Option<&MarketTicker>,
    symbol: Option<&str>,
    exchange: Option<&str>,
) -> Result<(String, String)> {
    if let Some(ticker) = ticker {
        Ok((ticker.symbol().to_string(), ticker.exchange().to_string()))
    } else if let Some(symbol) = symbol {
        if let Some(exchange) = exchange {
            Ok((symbol.to_string(), exchange.to_string()))
        } else {
            Err(Error::TradingViewError(TradingViewError::MissingExchange))
        }
    } else {
        Err(Error::TradingViewError(TradingViewError::MissingSymbol))
    }
}

/// Create callback handlers for processing incoming WebSocket data
fn create_callbacks(
    senders: Senders,
    replay_state: Arc<Mutex<ReplayState>>,
) -> TradingViewEventHandlers {
    let data_tx = Arc::clone(&senders.data);
    let info_tx = Arc::clone(&senders.info);
    let error_tx = Arc::clone(&senders.error);
    let completion_tx = Arc::clone(&senders.completion);

    TradingViewEventHandlers::default()
        .on_chart_data({
            let data_tx = Arc::clone(&data_tx);
            let replay_state = Arc::clone(&replay_state);
            move |(series_info, data_points): (SeriesInfo, Vec<DataPoint>)| {
                tracing::debug!("Received data batch with {} points", data_points.len());
                let tx = Arc::clone(&data_tx);
                let replay_state = Arc::clone(&replay_state);
                spawn(async move {
                    // Update replay state
                    {
                        let mut state = replay_state.lock().await;
                        state.data_received = true;
                        if state.enabled && !data_points.is_empty() {
                            let earliest =
                                data_points.iter().map(|p| p.timestamp()).min().unwrap_or(0);
                            state.earliest_ts = Some(
                                state
                                    .earliest_ts
                                    .map(|existing| existing.min(earliest))
                                    .unwrap_or(earliest),
                            );
                        }
                    }

                    let sender = tx.lock().await;
                    if let Err(e) = sender.send((series_info, data_points)).await {
                        tracing::error!("Failed to send data points: {}", e);
                    }
                });
            }
        })
        .on_symbol_info({
            let info_tx = Arc::clone(&info_tx);
            move |symbol_info| {
                tracing::debug!("Received symbol info: {:?}", symbol_info);
                let tx = Arc::clone(&info_tx);
                spawn(async move {
                    let sender = tx.lock().await;
                    if let Err(e) = sender.send(symbol_info).await {
                        tracing::error!("Failed to send symbol info: {}", e);
                    }
                });
            }
        })
        .on_series_completed({
            let completion_tx = Arc::clone(&completion_tx);
            let replay_state = Arc::clone(&replay_state);
            move |message: Vec<Value>| {
                let msg_json = serde_json::to_string(&message)
                    .unwrap_or_else(|_| "Failed to serialize message".to_string());
                let completion_tx = Arc::clone(&completion_tx);
                let replay_state = Arc::clone(&replay_state);

                tracing::debug!("Series completed with message: {:?}", message);
                spawn(async move {
                    let state = replay_state.lock().await;
                    let should_complete = if state.enabled {
                        msg_json.contains("replay") && msg_json.contains("data_completed")
                    } else {
                        true
                    };

                    if should_complete {
                        if let Some(sender) = completion_tx.lock().await.take() {
                            if let Err(e) = sender.send(CompletionSignal::Success) {
                                tracing::error!("Failed to send completion signal: {:?}", e);
                            }
                        }
                    }
                });
            }
        })
        .on_error({
            let completion_tx = Arc::clone(&completion_tx);
            let error_tx = Arc::clone(&error_tx);
            move |(error, message)| {
                tracing::error!("WebSocket error: {:?} - {:?}", error, message);

                let completion_tx = Arc::clone(&completion_tx);
                let error_tx = Arc::clone(&error_tx);

                spawn(async move {
                    let is_critical = is_critical_error(&error);
                    let err_msg = serde_json::to_string(&message)
                        .unwrap_or_else(|_| "Failed to serialize error message".to_string());

                    {
                        let sender = error_tx.lock().await;
                        if let Err(e) = sender.send((error, message)).await {
                            tracing::error!("Failed to send error: {}", e);
                        }
                    }

                    if is_critical {
                        tracing::error!("Critical error occurred, aborting all operations");
                        if let Some(sender) = completion_tx.lock().await.take() {
                            if let Err(e) = sender.send(CompletionSignal::Error(err_msg)) {
                                tracing::error!("Failed to send error completion signal: {:?}", e);
                            }
                        }
                    }
                });
            }
        })
}

fn is_critical_error(error: &Error) -> bool {
    match error {
        Error::LoginError(_) => true,
        Error::NoChartTokenFound => true,
        Error::WebSocketError(_) => true,
        Error::TradingViewError(e) => matches!(
            e,
            TradingViewError::CriticalError | TradingViewError::ProtocolError
        ),
        _ => false,
    }
}

/// Set up and initialize WebSocket connection with appropriate configuration
async fn setup_websocket(
    auth_token: &str,
    server: Option<DataServer>,
    callbacks: TradingViewEventHandlers,
    options: ChartOptions,
) -> Result<WebSocketClient> {
    let client = WebSocketHandler::default().set_callbacks(callbacks);

    let websocket = WebSocketClient::new()
        .server(server.unwrap_or(DataServer::ProData))
        .auth_token(auth_token)
        .client(client)
        .build()
        .await?;

    // Configure market settings before starting subscription
    websocket.set_market(options).await?;

    Ok(websocket)
}

/// Collect and accumulate incoming historical data from channels
async fn collect_data(
    receivers: Receivers,
    websocket: &WebSocketClient,
    replay_state: Arc<Mutex<ReplayState>>,
) -> Result<ChartHistoricalData> {
    let mut data = ChartHistoricalData::new();
    let mut completion_rx = receivers.completion.lock().await;
    let mut data_rx = receivers.data.lock().await;
    let mut info_rx = receivers.info.lock().await;
    let mut error_rx = receivers.error.lock().await;

    let mut replay_attempted = false;
    let mut last_error: Option<Error> = None;

    loop {
        tokio::select! {
            Some((series_info, data_points)) = data_rx.recv() => {
                tracing::debug!("Processing batch of {} data points", data_points.len());
                data.series_info = series_info;
                data.data.extend(data_points);

                // Handle replay mode setup
                if let Err(e) = handle_replay(
                    &replay_state,
                    &mut replay_attempted,
                    &data,
                    websocket
                ).await {
                    tracing::error!("Failed to setup replay mode: {}", e);
                    last_error = Some(e);
                }
            }

            completion_signal = &mut *completion_rx => {
                match completion_signal {
                    Ok(CompletionSignal::Success) => {
                        tracing::debug!("Completion signal received successfully");
                        break;
                    }
                    Ok(CompletionSignal::Error(error)) => {
                        tracing::error!("Completion with error: {}", error);
                        return Err(Error::Generic(error));
                    }
                    Ok(CompletionSignal::Timeout) => {
                        tracing::warn!("Operation timed out");
                        return Err(Error::TimeoutError("Data collection timed out".to_string()));
                    }
                    Err(_) => {
                        tracing::debug!("Completion channel closed");
                        break;
                    }
                }
            }

            Some(symbol_info) = info_rx.recv() => {
                tracing::debug!("Processing symbol info: {:?}", symbol_info);
                data.symbol_info = symbol_info;
            }

            Some((error, message)) = error_rx.recv() => {
                tracing::warn!("Non-critical error received: {:?} - {:?}", error, message);
                last_error = Some(error);
            }

            else => {
                tracing::debug!("All channels closed, no more data to receive");
                break;
            }
        }
    }

    // Process any remaining data points with timeout
    process_remaining(&mut data_rx, &mut info_rx, &mut data).await;

    // Return error if we had a critical issue and no data
    if data.data.is_empty() {
        if let Some(error) = last_error {
            return Err(error);
        }
    }

    Ok(data)
}

async fn handle_replay(
    replay_state: &Arc<Mutex<ReplayState>>,
    replay_attempted: &mut bool,
    data: &ChartHistoricalData,
    websocket: &WebSocketClient,
) -> Result<()> {
    let mut state = replay_state.lock().await;

    if state.enabled
        && !state.configured
        && !*replay_attempted
        && state.data_received
        && !data.data.is_empty()
    {
        tracing::debug!("Setting up replay mode");
        *replay_attempted = true;

        let earliest_ts: i64 = state.earliest_ts.unwrap_or_else(|| {
            data.data
                .iter()
                .map(|point| point.timestamp())
                .min()
                .unwrap_or(0)
        });

        let mut options = data.series_info.options.clone();
        options.replay_mode = true;
        options.replay_from = earliest_ts;

        tracing::debug!(
            "Setting replay mode with earliest timestamp: {}",
            earliest_ts
        );

        // Small delay to ensure data processing is complete
        sleep(Duration::from_millis(100)).await;

        websocket.set_market(options).await.map_err(|e| {
            tracing::error!("Failed to set replay mode: {}", e);
            e
        })?;

        state.configured = true;
        tracing::debug!("Replay mode configured successfully");
    }

    Ok(())
}

async fn process_remaining(
    data_rx: &mut mpsc::Receiver<(SeriesInfo, Vec<DataPoint>)>,
    info_rx: &mut mpsc::Receiver<SymbolInfo>,
    data: &mut ChartHistoricalData,
) {
    let timeout_dur = Duration::from_millis(200);

    // Process remaining data points
    while let Ok(Some((series_info, data_points))) = timeout(timeout_dur, data_rx.recv()).await {
        tracing::debug!(
            "Processing final batch of {} data points",
            data_points.len()
        );
        data.series_info = series_info;
        data.data.extend(data_points);
    }

    // Process remaining symbol info
    while let Ok(Some(symbol_info)) = timeout(timeout_dur, info_rx.recv()).await {
        tracing::debug!("Processing final symbol info: {:?}", symbol_info);
        data.symbol_info = symbol_info;
    }
}

async fn cleanup(websocket: Arc<WebSocketClient>, sub_task: tokio::task::JoinHandle<()>) {
    // Cancel subscription task
    sub_task.abort();

    // Give a moment for graceful shutdown
    if let Err(e) = timeout(Duration::from_millis(500), sub_task).await {
        tracing::debug!("Subscription task cleanup timeout: {:?}", e);
    }

    // Close WebSocket connection
    if let Err(e) = websocket.delete().await {
        tracing::error!("Failed to close WebSocket connection: {}", e);
    } else {
        tracing::debug!("WebSocket connection closed successfully");
    }
}
