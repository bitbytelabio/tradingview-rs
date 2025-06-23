use crate::{
    ChartHistoricalData, DataPoint, Error, Interval, MarketSymbol, OHLCV as _, Result, SymbolInfo,
    callback::EventCallback,
    chart::ChartOptions,
    error::TradingViewError,
    socket::DataServer,
    websocket::{SeriesInfo, WebSocket, WebSocketClient},
};
use bon::builder;
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};
use tokio::{
    spawn,
    sync::{Mutex, mpsc, oneshot},
    time::{sleep, timeout},
};

pub type DataPointSender = Arc<Mutex<mpsc::Sender<(SeriesInfo, Vec<DataPoint>)>>>;
pub type DataPointReceiver = Arc<Mutex<mpsc::Receiver<(SeriesInfo, Vec<DataPoint>)>>>;
pub type SymbolInfoSender = Arc<Mutex<mpsc::Sender<SymbolInfo>>>;
pub type SymbolInfoReceiver = Arc<Mutex<mpsc::Receiver<SymbolInfo>>>;

#[derive(Debug, Clone)]
struct CallbackSender {
    pub datapoint: DataPointSender,
    pub info: SymbolInfoSender,
    pub completion: Arc<Mutex<Option<oneshot::Sender<()>>>>,
}

#[derive(Debug, Clone)]
struct CallbackReceiver {
    pub datapoint: DataPointReceiver,
    pub info: SymbolInfoReceiver,
    pub completion: Arc<Mutex<oneshot::Receiver<()>>>,
}

#[derive(Debug, Clone)]
struct DataHandlers {
    pub sender: CallbackSender,
    pub receiver: CallbackReceiver,
}

impl DataHandlers {
    pub fn new() -> Self {
        let (data_sender, data_receiver) = mpsc::channel::<(SeriesInfo, Vec<DataPoint>)>(100);
        let (info_sender, info_receiver) = mpsc::channel::<SymbolInfo>(100);
        let (completion_sender, completion_receiver) = oneshot::channel::<()>();

        Self {
            sender: CallbackSender {
                datapoint: Arc::new(Mutex::new(data_sender)),
                info: Arc::new(Mutex::new(info_sender)),
                completion: Arc::new(Mutex::new(Some(completion_sender))),
            },
            receiver: CallbackReceiver {
                datapoint: Arc::new(Mutex::new(data_receiver)),
                info: Arc::new(Mutex::new(info_receiver)),
                completion: Arc::new(Mutex::new(completion_receiver)),
            },
        }
    }
}

/// Fetch historical chart data from TradingView
#[builder]
pub async fn fetch_chart_data(
    auth_token: Option<&str>,
    options: ChartOptions,
    server: Option<DataServer>,
    #[builder(default = false)] with_replay: bool,
) -> Result<ChartHistoricalData> {
    let auth_token = if auth_token.is_some() {
        auth_token.unwrap()
    } else {
        tracing::warn!("No auth token provided, using environment variable");
        &std::env::var("TV_AUTH_TOKEN").unwrap_or_else(|_| {
            tracing::error!("TV_AUTH_TOKEN environment variable is not set");
            panic!("TV_AUTH_TOKEN is required for data fetching");
        })
    };

    let data_handlers = DataHandlers::new();
    let set_replay = false;

    // Create callback handlers
    let callbacks = create_data_callbacks(data_handlers.sender.clone(), with_replay);

    // Initialize and configure WebSocket connection
    let websocket =
        setup_websocket_connection(auth_token, server, callbacks, options.clone()).await?;
    let websocket_shared = Arc::new(websocket);
    let websocket_shared_for_loop = Arc::clone(&websocket_shared);

    // Start the subscription process in a background task
    let subscription_task = spawn(async move {
        Arc::clone(&websocket_shared_for_loop).subscribe().await;
    });

    // Collect and process incoming data
    let result = collect_historical_data(
        data_handlers.receiver.clone(),
        &websocket_shared.clone(),
        with_replay,
        set_replay,
    )
    .await;

    // Cleanup resources
    if let Err(e) = websocket_shared.delete().await {
        tracing::error!("Failed to close WebSocket connection: {}", e);
    }
    subscription_task.abort();

    tracing::debug!(
        "Data collection completed with {} points",
        result.data.len()
    );
    Ok(result)
}

/// Create callback handlers for processing incoming WebSocket data
fn create_data_callbacks(sender: CallbackSender, with_replay: bool) -> EventCallback {
    let datapoint_sender = Arc::clone(&sender.datapoint);
    let info_sender = Arc::clone(&sender.info);
    let completion_sender = Arc::clone(&sender.completion);

    EventCallback::default()
        .on_chart_data({
            let datapoint_sender = Arc::clone(&datapoint_sender);
            move |(series_info, data_points): (SeriesInfo, Vec<DataPoint>)| {
                tracing::debug!("Received data batch with {} points", data_points.len());
                let sender = Arc::clone(&datapoint_sender);
                spawn(async move {
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
            move |message: Vec<Value>| {
                let message_json = serde_json::to_string(&message)
                    .unwrap_or_else(|_| "Failed to serialize message".to_string());
                let completion_sender = Arc::clone(&completion_sender);
                tracing::debug!("Series completed with message: {:?}", message);
                spawn(async move {
                    if !with_replay {
                        if let Some(sender) = completion_sender.lock().await.take() {
                            if let Err(e) = sender.send(()) {
                                tracing::error!("Failed to send completion signal: {:?}", e);
                            }
                        }
                    } else if message_json.contains("replay")
                        && message_json.contains("data_completed")
                    {
                        tracing::debug!("Replay data completed, sending completion signal");
                        if let Some(sender) = completion_sender.lock().await.take() {
                            if let Err(e) = sender.send(()) {
                                tracing::error!("Failed to send completion signal: {:?}", e);
                            }
                        }
                    }
                });
            }
        })
        .on_error({
            let completion_sender = Arc::clone(&completion_sender);
            move |(error, message)| {
                tracing::error!("WebSocket error: {:?}", message);
                // Check if this is a critical error that should abort the operation
                let is_critical = match &error {
                    Error::LoginError(_) => true,
                    Error::NoChartTokenFound => true,
                    Error::WebSocketError(_) => true,
                    Error::TradingViewError(e) => {
                        matches!(e, TradingViewError::CriticalError)
                    }
                    _ => false,
                };
                let completion_sender = Arc::clone(&completion_sender);
                spawn(async move {
                    if is_critical {
                        tracing::error!("Critical error occurred, aborting all operations");
                        if let Some(sender) = completion_sender.lock().await.take() {
                            if let Err(e) = sender.send(()) {
                                tracing::error!(
                                    "Failed to send completion signal on error: {:?}",
                                    e
                                );
                            }
                        }
                    }
                });
            }
        })
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
    with_replay: bool,
    mut set_replay: bool,
) -> ChartHistoricalData {
    let mut historical_data = ChartHistoricalData::new();
    let mut completion_receiver = receiver.completion.lock().await;

    // Process incoming data until completion signal or channel closure
    let mut data_receiver = receiver.datapoint.lock().await;
    let mut info_receiver = receiver.info.lock().await;
    loop {
        tokio::select! {
            Some((series_info, data_points)) = data_receiver.recv() => {
                tracing::debug!("Processing batch of {} data points", data_points.len());
                historical_data.series_info = series_info;
                historical_data.data.extend(data_points);
                // If replay mode is enabled, we can process data immediately
                // Handle replay mode setup after all data is collected
                if with_replay && !historical_data.data.is_empty() && !set_replay {
                    let mut options = historical_data.series_info.options.clone();
                    historical_data
                        .data
                        .sort_by(|a, b| a.timestamp().cmp(&b.timestamp()));
                    let earliest_timestamp = historical_data
                        .data
                        .first()
                        .map(|point| point.timestamp())
                        .unwrap_or(0);
                    options.replay_mode = true;
                    options.replay_from = earliest_timestamp;
                    tracing::debug!(
                        "Setting replay mode with earliest timestamp: {}",
                        earliest_timestamp
                    );
                    set_replay = true;
                    // Send the options to trigger replay mode
                    if let Err(e) = websocket.set_market(options).await {
                        tracing::error!("Failed to set options for replay: {}", e);
                    }
                }
            }
            _ = &mut *completion_receiver => {
                tracing::debug!("Completion signal received");
                // Process any remaining data points with timeout
                process_remaining_data_points(&mut data_receiver, &mut historical_data).await;
                process_remaining_symbol_info(&mut info_receiver, &mut historical_data).await;
                break;
            }
            Some(symbol_info) = info_receiver.recv() => {
                tracing::debug!("Processing symbol info: {:?}", symbol_info);
                historical_data.symbol_info = symbol_info;
            }
            else => {
                tracing::debug!("All channels closed, no more data to receive");
                break;
            }
        }
    }

    historical_data
}

async fn process_remaining_symbol_info(
    info_receiver: &mut mpsc::Receiver<SymbolInfo>,
    historical_data: &mut ChartHistoricalData,
) {
    // Use a short timeout to collect any in-flight data
    while let Ok(Some(symbol_info)) =
        timeout(Duration::from_millis(100), info_receiver.recv()).await
    {
        tracing::debug!("Processing final symbol info: {:?}", symbol_info);
        historical_data.symbol_info = symbol_info;
    }
}

/// Process any remaining data points in the channel after completion signal
async fn process_remaining_data_points(
    data_receiver: &mut mpsc::Receiver<(SeriesInfo, Vec<DataPoint>)>,
    historical_data: &mut ChartHistoricalData,
) {
    // Use a short timeout to collect any in-flight data
    while let Ok(Some((series_info, data_points))) =
        timeout(Duration::from_millis(100), data_receiver.recv()).await
    {
        tracing::debug!(
            "Processing final batch of {} data points",
            data_points.len()
        );
        historical_data.series_info = series_info;
        historical_data.data.extend(data_points);
    }
}

#[derive(Debug, Clone)]
struct BatchDataHandlers {
    pub sender: CallbackSender,
    pub receiver: CallbackReceiver,
    pub symbols_count: Arc<Mutex<usize>>,
    pub symbols_replay_count: Arc<Mutex<usize>>,
}

impl BatchDataHandlers {
    pub fn new(total_symbols: usize) -> Self {
        let (data_sender, data_receiver) = mpsc::channel::<(SeriesInfo, Vec<DataPoint>)>(100);
        let (info_sender, info_receiver) = mpsc::channel::<SymbolInfo>(100);
        let (completion_sender, completion_receiver) = oneshot::channel::<()>();

        Self {
            sender: CallbackSender {
                datapoint: Arc::new(Mutex::new(data_sender)),
                info: Arc::new(Mutex::new(info_sender)),
                completion: Arc::new(Mutex::new(Some(completion_sender))),
            },
            receiver: CallbackReceiver {
                datapoint: Arc::new(Mutex::new(data_receiver)),
                info: Arc::new(Mutex::new(info_receiver)),
                completion: Arc::new(Mutex::new(completion_receiver)),
            },
            symbols_count: Arc::new(Mutex::new(total_symbols)),
            symbols_replay_count: Arc::new(Mutex::new(total_symbols)),
        }
    }
}

/// Create callback handlers for batch processing with improved replay logic
fn create_batch_data_callbacks(handlers: BatchDataHandlers, with_replay: bool) -> EventCallback {
    let datapoint_sender = Arc::clone(&handlers.sender.datapoint);
    let info_sender = Arc::clone(&handlers.sender.info);
    let completion_sender = Arc::clone(&handlers.sender.completion);
    let symbols_count = Arc::clone(&handlers.symbols_count);
    let symbols_replay_count = Arc::clone(&handlers.symbols_replay_count);

    EventCallback::default()
        .on_chart_data({
            let datapoint_sender = Arc::clone(&datapoint_sender);
            move |(series_info, data_points): (SeriesInfo, Vec<DataPoint>)| {
                tracing::debug!(
                    "Received data batch with {} points for {}:{}",
                    data_points.len(),
                    series_info.options.exchange,
                    series_info.options.symbol
                );
                let sender = Arc::clone(&datapoint_sender);
                spawn(async move {
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
                tracing::debug!(
                    "Received symbol info: {} - {}",
                    symbol_info.id,
                    symbol_info.description
                );
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
            let symbols_count = Arc::clone(&symbols_count);
            let symbols_replay_count = Arc::clone(&symbols_replay_count);
            move |message: Vec<Value>| {
                let completion_sender = Arc::clone(&completion_sender);
                let symbols_count = Arc::clone(&symbols_count);
                let symbols_replay_count = Arc::clone(&symbols_replay_count);

                tracing::debug!("Series completed with message: {:?}", message);
                spawn(async move {
                    let message_json = serde_json::to_string(&message)
                        .unwrap_or_else(|_| "Failed to serialize message".to_string());

                    if with_replay
                        && message_json.contains("replay")
                        && message_json.contains("data_completed")
                    {
                        // Handle replay completion
                        let mut replay_count = symbols_replay_count.lock().await;
                        *replay_count -= 1;
                        tracing::debug!(
                            "Replay data completed, remaining replay symbols: {}",
                            *replay_count
                        );

                        if *replay_count == 0 {
                            tracing::debug!(
                                "All replay symbols processed, sending completion signal"
                            );
                            if let Some(sender) = completion_sender.lock().await.take() {
                                if let Err(e) = sender.send(()) {
                                    tracing::error!("Failed to send completion signal: {:?}", e);
                                }
                            }
                        }
                    } else {
                        // Handle regular series completion
                        let mut count = symbols_count.lock().await;
                        *count -= 1;
                        tracing::debug!("Series completed, remaining symbols: {}", *count);

                        if !with_replay && *count == 0 {
                            tracing::debug!("All symbols processed, sending completion signal");
                            if let Some(sender) = completion_sender.lock().await.take() {
                                if let Err(e) = sender.send(()) {
                                    tracing::error!("Failed to send completion signal: {:?}", e);
                                }
                            }
                        }
                    }
                });
            }
        })
        .on_error({
            let completion_sender = Arc::clone(&completion_sender);
            let symbols_count = Arc::clone(&symbols_count);
            move |(error, message)| {
                tracing::error!("WebSocket error: {:?}", message);
                let is_critical = match &error {
                    Error::LoginError(_) => true,
                    Error::NoChartTokenFound => true,
                    Error::WebSocketError(_) => true,
                    Error::TradingViewError(e) => {
                        matches!(e, TradingViewError::CriticalError)
                    }
                    _ => false,
                };

                let completion_sender = Arc::clone(&completion_sender);
                let symbols_count = Arc::clone(&symbols_count);
                spawn(async move {
                    if is_critical {
                        tracing::error!("Critical error occurred, aborting all operations");
                        if let Some(sender) = completion_sender.lock().await.take() {
                            if let Err(e) = sender.send(()) {
                                tracing::error!(
                                    "Failed to send completion signal on error: {:?}",
                                    e
                                );
                            }
                        }
                    } else {
                        // Decrement counter on non-critical errors
                        let mut count = symbols_count.lock().await;
                        *count -= 1;
                        if *count == 0 {
                            if let Some(sender) = completion_sender.lock().await.take() {
                                if let Err(e) = sender.send(()) {
                                    tracing::error!("Failed to send completion signal: {:?}", e);
                                }
                            }
                        }
                    }
                });
            }
        })
}

/// Setup WebSocket connection for batch processing with sequential market configuration
async fn setup_batch_websocket_connection(
    auth_token: &str,
    server: Option<DataServer>,
    callbacks: EventCallback,
    symbols: &[impl MarketSymbol],
    intervals: &[Interval],
    batch_size: usize,
) -> Result<WebSocket> {
    let client = WebSocketClient::default().set_callbacks(callbacks);

    let websocket = WebSocket::new()
        .server(server.unwrap_or(DataServer::ProData))
        .auth_token(auth_token)
        .client(client)
        .build()
        .await?;

    // Prepare all market options
    let mut options_list = Vec::new();
    for symbol in symbols {
        for &interval in intervals {
            let opt = ChartOptions::new_with(symbol.symbol(), symbol.exchange(), interval);
            options_list.push(opt);
        }
    }

    // Process market settings sequentially in batches
    for chunk in options_list.chunks(batch_size) {
        for opt in chunk {
            if let Err(e) = websocket.set_market(opt.clone()).await {
                tracing::error!(
                    "Failed to set market for {}:{}: {}",
                    opt.exchange,
                    opt.symbol,
                    e
                );
            }
        }

        // Wait between batches to avoid overwhelming the server
        if chunk.len() == batch_size {
            sleep(Duration::from_secs(2)).await;
            tracing::debug!(
                "Processed batch of {} markets, waiting before next batch",
                chunk.len()
            );
        }
    }

    tracing::debug!("Finished setting all {} markets", options_list.len());
    Ok(websocket)
}

/// Collect and process batch historical data with sequential replay handling
async fn collect_batch_historical_data(
    handlers: BatchDataHandlers,
    websocket: &WebSocket,
    with_replay: bool,
) -> HashMap<String, ChartHistoricalData> {
    let mut results = HashMap::new();
    let mut symbol_info_cache: HashMap<String, SymbolInfo> = HashMap::new(); // Cache symbol info until we get data
    let mut replay_set_for_symbols = HashSet::new();
    let mut completion_receiver = handlers.receiver.completion.lock().await;
    let mut data_receiver = handlers.receiver.datapoint.lock().await;
    let mut info_receiver = handlers.receiver.info.lock().await;

    loop {
        tokio::select! {
            Some((series_info, data_points)) = data_receiver.recv() => {
                let symbol_key = format!("{}:{}:{}",
                    series_info.options.exchange,
                    series_info.options.symbol,
                    series_info.options.interval
                );

                tracing::debug!("Processing batch for symbol {}: {} points", symbol_key, data_points.len());

                // Only create entry if we have actual data points
                if !data_points.is_empty() {
                    let historical_data = results.entry(symbol_key.clone())
                        .or_insert_with(ChartHistoricalData::new);

                    historical_data.series_info = series_info.clone();
                    historical_data.data.extend(data_points);

                    // Check if we have cached symbol info for this symbol
                    if let Some(symbol_info) = symbol_info_cache.remove(&symbol_key) {
                        historical_data.symbol_info = symbol_info;
                    } else {
                        // Try to match by partial key (symbol ID might be different format)
                        for (cached_key, cached_info) in symbol_info_cache.iter() {
                            if cached_key.contains(&series_info.options.symbol) ||
                               cached_info.id.contains(&series_info.options.symbol) {
                                historical_data.symbol_info = cached_info.clone();
                                break;
                            }
                        }
                    }

                    // Handle replay mode setup - similar to single symbol logic
                    if with_replay && !historical_data.data.is_empty() && !replay_set_for_symbols.contains(&symbol_key) {
                        let mut options = series_info.options.clone();
                        historical_data.data.sort_by(|a, b| a.timestamp().cmp(&b.timestamp()));

                        let earliest_timestamp = historical_data
                            .data
                            .first()
                            .map(|point| point.timestamp())
                            .unwrap_or(0);

                        options.replay_mode = true;
                        options.replay_from = earliest_timestamp;
                        replay_set_for_symbols.insert(symbol_key.clone());

                        tracing::debug!(
                            "Setting replay mode for {} with earliest timestamp: {}",
                            symbol_key,
                            earliest_timestamp
                        );

                        if let Err(e) = websocket.set_market(options).await {
                            tracing::error!("Failed to set options for replay on {}: {}", symbol_key, e);
                        }
                    }
                } else {
                    tracing::debug!("Skipping symbol {} - no data points received", symbol_key);
                }
            }
            Some(symbol_info) = info_receiver.recv() => {
                tracing::debug!("Processing symbol info: {:?}", symbol_info);

                // Try to find existing entry with data
                let mut matched = false;
                for (key, historical_data) in results.iter_mut() {
                    if key.contains(&symbol_info.id) ||
                       symbol_info.id.contains(&extract_symbol_from_key(key)) {
                        historical_data.symbol_info = symbol_info.clone();
                        matched = true;
                        break;
                    }
                }

                // If no existing entry with data, cache the symbol info
                if !matched {
                    // Create a potential key format to cache against
                    let potential_key = symbol_info.id.clone();
                    symbol_info_cache.insert(potential_key, symbol_info);
                }
            }
            _ = &mut *completion_receiver => {
                tracing::debug!("Completion signal received for batch processing");

                // Process any remaining data points with timeout
                process_remaining_batch_data(&mut data_receiver, &mut info_receiver, &mut results, &mut symbol_info_cache).await;
                break;
            }
            else => {
                tracing::debug!("All channels closed, no more data to receive");
                break;
            }
        }
    }

    // Log symbols that had info but no data
    if !symbol_info_cache.is_empty() {
        tracing::warn!(
            "Found {} symbols with info but no data points:",
            symbol_info_cache.len()
        );
        for (key, info) in &symbol_info_cache {
            tracing::warn!("  - {}: {} ({})", key, info.description, info.exchange);
        }
    }

    results
}
/// Extract symbol name from the composite key
fn extract_symbol_from_key(key: &str) -> &str {
    // Key format is "exchange:symbol:interval"
    key.split(':').nth(1).unwrap_or(key)
}

/// Process remaining data in batch channels after completion
async fn process_remaining_batch_data(
    data_receiver: &mut mpsc::Receiver<(SeriesInfo, Vec<DataPoint>)>,
    info_receiver: &mut mpsc::Receiver<SymbolInfo>,
    results: &mut HashMap<String, ChartHistoricalData>,
    symbol_info_cache: &mut HashMap<String, SymbolInfo>,
) {
    // Process remaining data points
    while let Ok(Some((series_info, data_points))) =
        timeout(Duration::from_millis(100), data_receiver.recv()).await
    {
        let symbol_key = format!(
            "{}:{}:{}",
            series_info.options.exchange, series_info.options.symbol, series_info.options.interval
        );

        tracing::debug!(
            "Processing final batch for {}: {} points",
            symbol_key,
            data_points.len()
        );

        // Only create entry if we have actual data points
        if !data_points.is_empty() {
            let historical_data = results
                .entry(symbol_key.clone())
                .or_insert_with(ChartHistoricalData::new);

            historical_data.series_info = series_info;
            historical_data.data.extend(data_points);

            // Check if we have cached symbol info for this symbol
            if let Some(symbol_info) = symbol_info_cache.remove(&symbol_key) {
                historical_data.symbol_info = symbol_info;
            }
        }
    }

    // Process remaining symbol info - only cache, don't create entries without data
    while let Ok(Some(symbol_info)) =
        timeout(Duration::from_millis(100), info_receiver.recv()).await
    {
        tracing::debug!("Processing final symbol info: {:?}", symbol_info);

        // Try to match with existing data entries
        let mut matched = false;
        for (key, historical_data) in results.iter_mut() {
            if key.contains(&symbol_info.id)
                || symbol_info.id.contains(&extract_symbol_from_key(key))
            {
                historical_data.symbol_info = symbol_info.clone();
                matched = true;
                break;
            }
        }

        // Cache if no match found
        if !matched {
            symbol_info_cache.insert(symbol_info.id.clone(), symbol_info);
        }
    }
}

#[builder]
pub async fn fetch_chart_data_batch(
    auth_token: Option<&str>,
    symbols: &[impl MarketSymbol],
    interval: &[Interval],
    #[builder(default = false)] with_replay: bool,
    server: Option<DataServer>,
    #[builder(default = 10)] batch_size: usize,
) -> Result<HashMap<String, ChartHistoricalData>> {
    let auth_token = if let Some(token) = auth_token {
        token
    } else {
        tracing::warn!("No auth token provided, using environment variable");
        &std::env::var("TV_AUTH_TOKEN").unwrap_or_else(|_| {
            tracing::error!("TV_AUTH_TOKEN environment variable is not set");
            panic!("TV_AUTH_TOKEN is required for data fetching");
        })
    };

    let total_symbols = symbols.len() * interval.len();
    let data_handlers = BatchDataHandlers::new(total_symbols);

    // Create callback handlers using the improved logic
    let callbacks = create_batch_data_callbacks(data_handlers.clone(), with_replay);

    // Setup WebSocket connection with sequential processing
    let websocket = setup_batch_websocket_connection(
        auth_token, server, callbacks, symbols, interval, batch_size,
    )
    .await?;

    let websocket_shared = Arc::new(websocket);
    let websocket_shared_for_loop = Arc::clone(&websocket_shared);

    // Start the subscription process in a background task
    let subscription_task = spawn(async move {
        websocket_shared_for_loop.subscribe().await;
    });

    // Collect and process incoming data using improved logic
    let results =
        collect_batch_historical_data(data_handlers, &websocket_shared, with_replay).await;

    // Cleanup resources
    if let Err(e) = websocket_shared.delete().await {
        tracing::error!("Failed to close WebSocket connection: {}", e);
    }
    subscription_task.abort();

    tracing::debug!(
        "Batch data collection completed with {} symbols",
        results.len()
    );
    Ok(results)
}

#[cfg(test)]
#[allow(unused)]
mod tests {
    use super::*;
    use crate::{Country, Interval, MarketType, StocksType, chart::data, list_symbols};
    use anyhow::Ok;
    use std::{sync::Once, vec};

    fn init() {
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            tracing_subscriber::fmt()
                .with_max_level(tracing::Level::DEBUG)
                .init();
        });
    }

    #[tokio::test]
    async fn test_fetch_chart_historical() -> anyhow::Result<()> {
        init();
        dotenv::dotenv().ok();
        let auth_token = std::env::var("TV_AUTH_TOKEN").expect("TV_AUTH_TOKEN is not set");
        let symbol = "VCB";
        let exchange = "HOSE";
        let interval = Interval::OneDay;
        let option = ChartOptions::new_with(symbol, exchange, interval).bar_count(10);
        let data = fetch_chart_data()
            .auth_token(&auth_token)
            .options(option)
            .server(DataServer::ProData)
            .call()
            .await?;
        assert!(!data.data.is_empty(), "Data should not be empty");
        assert_eq!(data.data.len(), 10, "Data length should be 10");
        assert_eq!(data.symbol_info.id, "HOSE:VCB", "Symbol should match");
        assert_eq!(data.series_info.options.symbol, symbol, "Name should match");
        assert_eq!(
            data.series_info.options.interval, interval,
            "Interval should match"
        );
        assert_eq!(
            data.series_info.options.exchange, exchange,
            "Exchange should match"
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_batch_fetch_chart_historical() -> anyhow::Result<()> {
        init();
        dotenv::dotenv().ok();
        let symbols = list_symbols()
            .exchange("HNX")
            .market_type(MarketType::Stocks(StocksType::Common))
            .country(Country::VN)
            .call()
            .await?;
        assert!(!symbols.is_empty(), "No symbols found");
        let symbols = symbols[0..50].to_vec();

        println!("Fetched {} symbols", symbols.len());
        let auth_token = std::env::var("TV_AUTH_TOKEN").expect("TV_AUTH_TOKEN is not set");
        let based_opt = ChartOptions::builder()
            .interval(Interval::OneDay)
            .bar_count(10)
            .build();

        let server = Some(DataServer::ProData);
        let data: HashMap<String, ChartHistoricalData> = fetch_chart_data_batch()
            .auth_token(&auth_token)
            .symbols(&symbols)
            .interval(&[Interval::OneDay])
            .batch_size(40)
            .with_replay(false)
            .call()
            .await?;

        for (k, d) in data {
            println!(
                "key: {}, Info: {:?}, Data Len: {}",
                k,
                d.symbol_info.id,
                d.data.len()
            );
        }

        Ok(())
    }
}
