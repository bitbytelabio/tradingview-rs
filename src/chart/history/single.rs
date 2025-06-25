use crate::{
    ChartHistoricalData, DataPoint, Error, Interval, MarketSymbol, MarketTicker, OHLCV as _,
    Result, SymbolInfo,
    callback::EventCallback,
    chart::ChartOptions,
    error::TradingViewError,
    history::resolve_auth_token,
    options::Range,
    socket::DataServer,
    websocket::{SeriesInfo, WebSocket, WebSocketClient},
};
use bon::builder;
use serde_json::Value;
use std::{sync::Arc, time::Duration};
use tokio::{
    spawn,
    sync::{Mutex, mpsc, oneshot},
    time::{sleep, timeout},
};

pub type DataPointSender = Arc<Mutex<mpsc::Sender<(SeriesInfo, Vec<DataPoint>)>>>;
pub type DataPointReceiver = Arc<Mutex<mpsc::Receiver<(SeriesInfo, Vec<DataPoint>)>>>;
pub type SymbolInfoSender = Arc<Mutex<mpsc::Sender<SymbolInfo>>>;
pub type SymbolInfoReceiver = Arc<Mutex<mpsc::Receiver<SymbolInfo>>>;
pub type ErrorSender = Arc<Mutex<mpsc::Sender<(Error, Vec<Value>)>>>;
pub type ErrorReceiver = Arc<Mutex<mpsc::Receiver<(Error, Vec<Value>)>>>;

#[derive(Debug)]
pub enum CompletionSignal {
    Success,
    Error(String),
    Timeout,
}

#[derive(Debug, Clone)]
struct CallbackSender {
    pub datapoint: DataPointSender,
    pub info: SymbolInfoSender,
    pub error: ErrorSender,
    pub completion: Arc<Mutex<Option<oneshot::Sender<CompletionSignal>>>>,
}

#[derive(Debug, Clone)]
struct CallbackReceiver {
    pub datapoint: DataPointReceiver,
    pub info: SymbolInfoReceiver,
    pub error: ErrorReceiver,
    pub completion: Arc<Mutex<oneshot::Receiver<CompletionSignal>>>,
}

#[derive(Debug, Clone)]
struct DataHandlers {
    pub sender: CallbackSender,
    pub receiver: CallbackReceiver,
}

impl DataHandlers {
    pub fn new() -> Self {
        let (data_sender, data_receiver) = mpsc::channel::<(SeriesInfo, Vec<DataPoint>)>(2000);
        let (info_sender, info_receiver) = mpsc::channel::<SymbolInfo>(2000);
        let (error_sender, error_receiver) = mpsc::channel::<(Error, Vec<Value>)>(2000);
        let (completion_sender, completion_receiver) = oneshot::channel::<CompletionSignal>();

        Self {
            sender: CallbackSender {
                datapoint: Arc::new(Mutex::new(data_sender)),
                info: Arc::new(Mutex::new(info_sender)),
                error: Arc::new(Mutex::new(error_sender)),
                completion: Arc::new(Mutex::new(Some(completion_sender))),
            },
            receiver: CallbackReceiver {
                datapoint: Arc::new(Mutex::new(data_receiver)),
                info: Arc::new(Mutex::new(info_receiver)),
                error: Arc::new(Mutex::new(error_receiver)),
                completion: Arc::new(Mutex::new(completion_receiver)),
            },
        }
    }
}

#[derive(Debug, Clone)]
struct ReplayState {
    pub enabled: bool,
    pub configured: bool,
    pub earliest_timestamp: Option<i64>,
    pub data_received: bool,
}

impl Default for ReplayState {
    fn default() -> Self {
        Self {
            enabled: false,
            configured: false,
            earliest_timestamp: None,
            data_received: false,
        }
    }
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
) -> Result<ChartHistoricalData> {
    let auth_token = resolve_auth_token(auth_token)?;
    let range = range.map(String::from);

    let (symbol, exchange) = validate_and_extract_symbol_exchange(ticker, symbol, exchange)?;

    let options = ChartOptions::builder()
        .symbol(symbol)
        .exchange(exchange)
        .interval(interval)
        .maybe_range(range)
        .maybe_bar_count(num_bars)
        .replay_mode(false) // Start without replay mode
        .build();

    let data_handlers = DataHandlers::new();
    let replay_state = Arc::new(Mutex::new(ReplayState {
        enabled: with_replay,
        ..Default::default()
    }));

    // Create callback handlers
    let callbacks = create_data_callbacks(data_handlers.sender.clone(), Arc::clone(&replay_state));

    // Initialize and configure WebSocket connection
    let websocket = setup_websocket_connection(&auth_token, server, callbacks, options).await?;
    let websocket_shared = Arc::new(websocket);

    // Start the subscription process in a background task
    let websocket_for_subscription = Arc::clone(&websocket_shared);
    let subscription_task = spawn(async move { websocket_for_subscription.subscribe().await });

    // Collect and process incoming data with timeout
    let collection_result = timeout(
        timeout_duration,
        collect_historical_data(data_handlers.receiver, &websocket_shared, replay_state),
    )
    .await;

    // Cleanup resources
    cleanup_resources(websocket_shared, subscription_task).await;

    match collection_result {
        Ok(Ok(result)) => {
            tracing::debug!(
                "Data collection completed with {} points",
                result.data.len()
            );
            Ok(result)
        }
        Ok(Err(error)) => Err(error),
        Err(_) => Err(Error::TimeoutError("Data collection timed out".to_string())),
    }
}

fn validate_and_extract_symbol_exchange(
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
fn create_data_callbacks(
    sender: CallbackSender,
    replay_state: Arc<Mutex<ReplayState>>,
) -> EventCallback {
    let datapoint_sender = Arc::clone(&sender.datapoint);
    let info_sender = Arc::clone(&sender.info);
    let error_sender = Arc::clone(&sender.error);
    let completion_sender = Arc::clone(&sender.completion);

    EventCallback::default()
        .on_chart_data({
            let datapoint_sender = Arc::clone(&datapoint_sender);
            let replay_state = Arc::clone(&replay_state);
            move |(series_info, data_points): (SeriesInfo, Vec<DataPoint>)| {
                tracing::debug!("Received data batch with {} points", data_points.len());
                let sender = Arc::clone(&datapoint_sender);
                let replay_state = Arc::clone(&replay_state);
                spawn(async move {
                    // Update replay state
                    {
                        let mut state = replay_state.lock().await;
                        state.data_received = true;
                        if state.enabled && !data_points.is_empty() {
                            let earliest =
                                data_points.iter().map(|p| p.timestamp()).min().unwrap_or(0);

                            state.earliest_timestamp = Some(
                                state
                                    .earliest_timestamp
                                    .map(|existing| existing.min(earliest))
                                    .unwrap_or(earliest),
                            );
                        }
                    }

                    let tx = sender.lock().await;
                    if let Err(e) = tx.send((series_info, data_points)).await {
                        tracing::error!("Failed to send data points to processing channel: {}", e);
                    }
                });
            }
        })
        .on_symbol_info({
            let info_sender = Arc::clone(&info_sender);
            move |symbol_info| {
                tracing::debug!("Received symbol info: {:?}", symbol_info);
                let sender = Arc::clone(&info_sender);
                spawn(async move {
                    let tx = sender.lock().await;
                    if let Err(e) = tx.send(symbol_info).await {
                        tracing::error!("Failed to send symbol info to processing channel: {}", e);
                    }
                });
            }
        })
        .on_series_completed({
            let completion_sender = Arc::clone(&completion_sender);
            let replay_state = Arc::clone(&replay_state);
            move |message: Vec<Value>| {
                let message_json = serde_json::to_string(&message)
                    .unwrap_or_else(|_| "Failed to serialize message".to_string());
                let completion_sender = Arc::clone(&completion_sender);
                let replay_state = Arc::clone(&replay_state);

                tracing::debug!("Series completed with message: {:?}", message);
                spawn(async move {
                    let state = replay_state.lock().await;
                    let should_complete = if state.enabled {
                        message_json.contains("replay") && message_json.contains("data_completed")
                    } else {
                        true
                    };

                    if should_complete {
                        if let Some(sender) = completion_sender.lock().await.take() {
                            if let Err(e) = sender.send(CompletionSignal::Success) {
                                tracing::error!("Failed to send completion signal: {:?}", e);
                            }
                        }
                    }
                });
            }
        })
        .on_error({
            let completion_sender = Arc::clone(&completion_sender);
            let error_sender = Arc::clone(&error_sender);
            move |(error, message)| {
                tracing::error!("WebSocket error: {:?} - {:?}", error, message);

                let completion_sender = Arc::clone(&completion_sender);
                let error_sender = Arc::clone(&error_sender);

                spawn(async move {
                    let is_critical = is_critical_error(&error);
                    let err_message = serde_json::to_string(&message)
                        .unwrap_or_else(|_| "Failed to serialize error message".to_string());

                    {
                        let tx = error_sender.lock().await;
                        if let Err(e) = tx.send((error, message)).await {
                            tracing::error!("Failed to send error to processing channel: {}", e);
                        }
                    }

                    if is_critical {
                        tracing::error!("Critical error occurred, aborting all operations");
                        if let Some(sender) = completion_sender.lock().await.take() {
                            if let Err(e) = sender.send(CompletionSignal::Error(err_message)) {
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
        Error::TradingViewError(e) => matches!(e, TradingViewError::CriticalError),
        _ => false,
    }
}

/// Set up and initialize WebSocket connection with appropriate configuration
async fn setup_websocket_connection(
    auth_token: &str,
    server: Option<DataServer>,
    callbacks: EventCallback,
    options: ChartOptions,
) -> Result<WebSocket> {
    let client = WebSocketClient::default().set_callbacks(callbacks);

    let websocket = WebSocket::new()
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
async fn collect_historical_data(
    receiver: CallbackReceiver,
    websocket: &WebSocket,
    replay_state: Arc<Mutex<ReplayState>>,
) -> Result<ChartHistoricalData> {
    let mut historical_data = ChartHistoricalData::new();
    let mut completion_receiver = receiver.completion.lock().await;
    let mut data_receiver = receiver.datapoint.lock().await;
    let mut info_receiver = receiver.info.lock().await;
    let mut error_receiver = receiver.error.lock().await;

    let mut replay_setup_attempted = false;
    let mut last_error: Option<Error> = None;

    loop {
        tokio::select! {
            Some((series_info, data_points)) = data_receiver.recv() => {
                tracing::debug!("Processing batch of {} data points", data_points.len());
                historical_data.series_info = series_info;
                historical_data.data.extend(data_points);

                // Handle replay mode setup
                if let Err(e) = handle_replay_setup(
                    &replay_state,
                    &mut replay_setup_attempted,
                    &historical_data,
                    websocket
                ).await {
                    tracing::error!("Failed to setup replay mode: {}", e);
                    last_error = Some(e);
                }
            }

            completion_signal = &mut *completion_receiver => {
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

            Some(symbol_info) = info_receiver.recv() => {
                tracing::debug!("Processing symbol info: {:?}", symbol_info);
                historical_data.symbol_info = symbol_info;
            }

            Some((error, message)) = error_receiver.recv() => {
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
    process_remaining_data(&mut data_receiver, &mut info_receiver, &mut historical_data).await;

    // Return error if we had a critical issue and no data
    if historical_data.data.is_empty() {
        if let Some(error) = last_error {
            return Err(error);
        }
    }

    Ok(historical_data)
}

async fn handle_replay_setup(
    replay_state: &Arc<Mutex<ReplayState>>,
    replay_setup_attempted: &mut bool,
    historical_data: &ChartHistoricalData,
    websocket: &WebSocket,
) -> Result<()> {
    let mut state = replay_state.lock().await;

    if state.enabled
        && !state.configured
        && !*replay_setup_attempted
        && state.data_received
        && !historical_data.data.is_empty()
    {
        tracing::debug!("Setting up replay mode");
        *replay_setup_attempted = true;

        let earliest_timestamp: i64 = state.earliest_timestamp.unwrap_or_else(|| {
            historical_data
                .data
                .iter()
                .map(|point| point.timestamp())
                .min()
                .unwrap_or(0)
        });

        let mut options = historical_data.series_info.options.clone();
        options.replay_mode = true;
        options.replay_from = earliest_timestamp;

        tracing::debug!(
            "Setting replay mode with earliest timestamp: {}",
            earliest_timestamp
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

async fn process_remaining_data(
    data_receiver: &mut mpsc::Receiver<(SeriesInfo, Vec<DataPoint>)>,
    info_receiver: &mut mpsc::Receiver<SymbolInfo>,
    historical_data: &mut ChartHistoricalData,
) {
    let processing_timeout = Duration::from_millis(200);

    // Process remaining data points
    while let Ok(Some((series_info, data_points))) =
        timeout(processing_timeout, data_receiver.recv()).await
    {
        tracing::debug!(
            "Processing final batch of {} data points",
            data_points.len()
        );
        historical_data.series_info = series_info;
        historical_data.data.extend(data_points);
    }

    // Process remaining symbol info
    while let Ok(Some(symbol_info)) = timeout(processing_timeout, info_receiver.recv()).await {
        tracing::debug!("Processing final symbol info: {:?}", symbol_info);
        historical_data.symbol_info = symbol_info;
    }
}

async fn cleanup_resources(
    websocket: Arc<WebSocket>,
    subscription_task: tokio::task::JoinHandle<()>,
) {
    // Cancel subscription task
    subscription_task.abort();

    // Give a moment for graceful shutdown
    if let Err(e) = timeout(Duration::from_millis(500), subscription_task).await {
        tracing::debug!("Subscription task cleanup timeout: {:?}", e);
    }

    // Close WebSocket connection
    if let Err(e) = websocket.delete().await {
        tracing::error!("Failed to close WebSocket connection: {}", e);
    } else {
        tracing::debug!("WebSocket connection closed successfully");
    }
}
