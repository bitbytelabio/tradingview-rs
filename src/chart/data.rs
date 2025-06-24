use crate::{
    ChartHistoricalData, DataPoint, Error, Interval, MarketSymbol, OHLCV as _, Result, SymbolInfo,
    callback::EventCallback,
    chart::ChartOptions,
    error::{LoginError, TradingViewError},
    socket::DataServer,
    websocket::{SeriesInfo, WebSocket, WebSocketClient},
};
use bon::builder;
use serde_json::Value;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::{
    spawn,
    sync::{Mutex, mpsc, oneshot},
    time::{sleep, timeout},
};

// Type aliases for better readability
type DataChannel = mpsc::Sender<(SeriesInfo, Vec<DataPoint>)>;
type InfoChannel = mpsc::Sender<SymbolInfo>;
type CompletionChannel = oneshot::Sender<()>;

// Configuration constants
const DATA_CHANNEL_BUFFER: usize = 1000;
const INFO_CHANNEL_BUFFER: usize = 100;
const REMAINING_DATA_TIMEOUT: Duration = Duration::from_millis(100);
const BATCH_PROCESSING_DELAY: Duration = Duration::from_secs(5);
const INTERVAL_PROCESSING_DELAY: Duration = Duration::from_secs(2);
const DEFAULT_BATCH_SIZE: usize = 15;

/// Handles for managing data communication channels
#[derive(Debug)]
struct DataChannels {
    data_tx: Arc<Mutex<DataChannel>>,
    info_tx: Arc<Mutex<InfoChannel>>,
    completion_tx: Arc<Mutex<Option<CompletionChannel>>>,
    data_rx: Arc<Mutex<mpsc::Receiver<(SeriesInfo, Vec<DataPoint>)>>>,
    info_rx: Arc<Mutex<mpsc::Receiver<SymbolInfo>>>,
    completion_rx: Arc<Mutex<oneshot::Receiver<()>>>,
}

impl DataChannels {
    fn new() -> Self {
        let (data_tx, data_rx) = mpsc::channel(DATA_CHANNEL_BUFFER);
        let (info_tx, info_rx) = mpsc::channel(INFO_CHANNEL_BUFFER);
        let (completion_tx, completion_rx) = oneshot::channel();

        Self {
            data_tx: Arc::new(Mutex::new(data_tx)),
            info_tx: Arc::new(Mutex::new(info_tx)),
            completion_tx: Arc::new(Mutex::new(Some(completion_tx))),
            data_rx: Arc::new(Mutex::new(data_rx)),
            info_rx: Arc::new(Mutex::new(info_rx)),
            completion_rx: Arc::new(Mutex::new(completion_rx)),
        }
    }
}

/// Configuration for batch processing
#[derive(Debug, Clone)]
struct BatchConfig {
    batch_size: usize,
    processing_delay: Duration,
    interval_delay: Duration,
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            batch_size: DEFAULT_BATCH_SIZE,
            processing_delay: BATCH_PROCESSING_DELAY,
            interval_delay: INTERVAL_PROCESSING_DELAY,
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
    let auth_token = resolve_auth_token(auth_token)?;
    let channels = DataChannels::new();

    // Create and configure WebSocket connection
    let callbacks = create_single_data_callbacks(&channels, with_replay);
    let websocket = setup_websocket_connection(&auth_token, server, callbacks, options).await?;
    let websocket = Arc::new(websocket);

    // Start subscription in background
    let subscription_handle = start_subscription_task(Arc::clone(&websocket));

    // Collect data with proper error handling
    let result = collect_historical_data(&channels, &websocket, with_replay).await;

    // Cleanup
    cleanup_resources(websocket, subscription_handle).await;

    match result {
        Ok(data) => {
            tracing::debug!("Data collection completed with {} points", data.data.len());
            Ok(data)
        }
        Err(e) => {
            tracing::error!("Failed to collect historical data: {}", e);
            Err(e)
        }
    }
}

/// Fetch chart data for multiple symbols in batches
#[builder]
pub async fn fetch_chart_data_batch(
    auth_token: Option<&str>,
    symbols: &[impl MarketSymbol],
    interval: &[Interval],
    server: Option<DataServer>,
    #[builder(default = 30)] batch_size: usize,
) -> Result<HashMap<String, ChartHistoricalData>> {
    let auth_token = resolve_auth_token(auth_token)?;
    let config = BatchConfig {
        batch_size,
        ..Default::default()
    };

    let mut results = HashMap::with_capacity(symbols.len() * interval.len());

    for symbol_chunk in symbols.chunks(config.batch_size) {
        tracing::debug!("Processing batch of {} symbols", symbol_chunk.len());

        for &interval_val in interval {
            match process_batch_interval(&auth_token, symbol_chunk, interval_val, server, &config)
                .await
            {
                Ok(batch_data) => {
                    tracing::debug!(
                        "Successfully processed {} symbols for interval {:?}",
                        batch_data.len(),
                        interval_val
                    );
                    results.extend(batch_data);
                }
                Err(e) => {
                    tracing::error!(
                        "Failed to process batch for interval {:?}: {}",
                        interval_val,
                        e
                    );
                    // Continue with other intervals instead of failing completely
                    continue;
                }
            }

            sleep(config.interval_delay).await;
        }

        sleep(config.processing_delay).await;
    }

    if results.is_empty() {
        tracing::warn!("No data collected for any symbols");
        return Err(Error::NoChartTokenFound);
    }

    tracing::info!(
        "Batch processing completed: {} total datasets",
        results.len()
    );
    Ok(results)
}

/// Resolve authentication token from parameter or environment
fn resolve_auth_token(auth_token: Option<&str>) -> Result<String> {
    match auth_token {
        Some(token) => Ok(token.to_string()),
        None => {
            tracing::warn!("No auth token provided, using environment variable");
            std::env::var("TV_AUTH_TOKEN").map_err(|_| {
                tracing::error!("TV_AUTH_TOKEN environment variable is not set");
                Error::LoginError(LoginError::InvalidSession)
            })
        }
    }
}

/// Process a single batch for a specific interval
async fn process_batch_interval(
    auth_token: &str,
    symbols: &[impl MarketSymbol],
    interval: Interval,
    server: Option<DataServer>,
    config: &BatchConfig,
) -> Result<HashMap<String, ChartHistoricalData>> {
    let options = ChartOptions::builder().interval(interval).build();

    tracing::debug!(
        "Processing {} symbols for interval {:?}",
        symbols.len(),
        interval
    );

    fetch_chart_data_batch_inner(auth_token, symbols, options, server, config.batch_size).await
}

/// Create callbacks for single symbol data fetching
fn create_single_data_callbacks(channels: &DataChannels, with_replay: bool) -> EventCallback {
    let data_tx = Arc::clone(&channels.data_tx);
    let info_tx = Arc::clone(&channels.info_tx);
    let completion_tx = Arc::clone(&channels.completion_tx);

    EventCallback::default()
        .on_chart_data(create_chart_data_handler(data_tx))
        .on_symbol_info(create_symbol_info_handler(info_tx))
        .on_series_completed(create_series_completion_handler(completion_tx, with_replay))
        .on_error(create_error_handler(Arc::clone(&channels.completion_tx)))
}

/// Create chart data handler
fn create_chart_data_handler(
    data_tx: Arc<Mutex<DataChannel>>,
) -> impl Fn((SeriesInfo, Vec<DataPoint>)) + Send + Sync + 'static {
    move |(series_info, data_points): (SeriesInfo, Vec<DataPoint>)| {
        tracing::debug!("Received data batch with {} points", data_points.len());
        let sender = Arc::clone(&data_tx);
        spawn(async move {
            if let Ok(tx) = sender.try_lock() {
                if let Err(e) = tx.send((series_info, data_points)).await {
                    tracing::error!("Failed to send data points: {}", e);
                }
            } else {
                tracing::warn!("Data channel is locked, skipping batch");
            }
        });
    }
}

/// Create symbol info handler
fn create_symbol_info_handler(
    info_tx: Arc<Mutex<InfoChannel>>,
) -> impl Fn(SymbolInfo) + Send + Sync + 'static {
    move |symbol_info| {
        tracing::debug!("Received symbol info: {:?}", symbol_info);
        let sender = Arc::clone(&info_tx);
        spawn(async move {
            if let Ok(tx) = sender.try_lock() {
                if let Err(e) = tx.send(symbol_info).await {
                    tracing::error!("Failed to send symbol info: {}", e);
                }
            } else {
                tracing::warn!("Info channel is locked, skipping symbol info");
            }
        });
    }
}

/// Create series completion handler
fn create_series_completion_handler(
    completion_tx: Arc<Mutex<Option<CompletionChannel>>>,
    with_replay: bool,
) -> impl Fn(Vec<Value>) + Send + Sync + 'static {
    move |message: Vec<Value>| {
        let completion_tx = Arc::clone(&completion_tx);
        let should_complete = if with_replay {
            let message_json = serde_json::to_string(&message).unwrap_or_default();
            message_json.contains("replay") && message_json.contains("data_completed")
        } else {
            true
        };

        if should_complete {
            spawn(async move {
                send_completion_signal(completion_tx, "Series completed").await;
            });
        }
    }
}

/// Create error handler
fn create_error_handler(
    completion_tx: Arc<Mutex<Option<CompletionChannel>>>,
) -> impl Fn((Error, Vec<Value>)) + Send + Sync + 'static {
    move |(error, message)| {
        tracing::error!("WebSocket error: {:?}", message);

        let is_critical = matches!(
            error,
            Error::LoginError(_)
                | Error::NoChartTokenFound
                | Error::WebSocketError(_)
                | Error::TradingViewError(
                    TradingViewError::CriticalError | TradingViewError::ProtocolError
                )
        );

        if is_critical {
            let completion_tx = Arc::clone(&completion_tx);
            spawn(async move {
                send_completion_signal(completion_tx, "Critical error occurred").await;
            });
        }
    }
}

/// Send completion signal safely
async fn send_completion_signal(
    completion_tx: Arc<Mutex<Option<CompletionChannel>>>,
    reason: &str,
) {
    if let Ok(mut tx_opt) = completion_tx.try_lock() {
        if let Some(tx) = tx_opt.take() {
            tracing::debug!("{}, sending completion signal", reason);
            if let Err(_) = tx.send(()) {
                tracing::warn!("Completion receiver was dropped");
            }
        }
    }
}

/// Set up WebSocket connection
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

    websocket.set_market(options).await?;
    Ok(websocket)
}

/// Start subscription task
fn start_subscription_task(websocket: Arc<WebSocket>) -> tokio::task::JoinHandle<()> {
    spawn(async move {
        websocket.subscribe().await;
    })
}

/// Collect historical data from channels
async fn collect_historical_data(
    channels: &DataChannels,
    websocket: &Arc<WebSocket>,
    with_replay: bool,
) -> Result<ChartHistoricalData> {
    let mut historical_data = ChartHistoricalData::new();
    let mut completion_rx = channels.completion_rx.lock().await;
    let mut data_rx = channels.data_rx.lock().await;
    let mut info_rx = channels.info_rx.lock().await;
    let mut set_replay = false;

    loop {
        tokio::select! {
            Some((series_info, data_points)) = data_rx.recv() => {
                handle_data_batch(&mut historical_data, series_info, data_points,
                    websocket, with_replay, &mut set_replay).await?;
            }
            Some(symbol_info) = info_rx.recv() => {
                tracing::debug!("Processing symbol info: {:?}", symbol_info);
                historical_data.symbol_info = symbol_info;
            }
            _ = &mut *completion_rx => {
                tracing::debug!("Completion signal received");
                process_remaining_data(&mut data_rx, &mut info_rx, &mut historical_data).await;
                break;
            }
            else => {
                tracing::debug!("All channels closed");
                break;
            }
        }
    }

    Ok(historical_data)
}

/// Handle incoming data batch
async fn handle_data_batch(
    historical_data: &mut ChartHistoricalData,
    series_info: SeriesInfo,
    data_points: Vec<DataPoint>,
    websocket: &Arc<WebSocket>,
    with_replay: bool,
    set_replay: &mut bool,
) -> Result<()> {
    tracing::debug!("Processing batch of {} data points", data_points.len());

    historical_data.series_info = series_info;
    historical_data.data.extend(data_points);

    // Handle replay mode setup
    if with_replay && !historical_data.data.is_empty() && !*set_replay {
        setup_replay_mode(historical_data, websocket).await?;
        *set_replay = true;
    }

    Ok(())
}

/// Setup replay mode
async fn setup_replay_mode(
    historical_data: &mut ChartHistoricalData,
    websocket: &Arc<WebSocket>,
) -> Result<()> {
    historical_data.data.sort_by_key(|point| point.timestamp());

    let earliest_timestamp = historical_data
        .data
        .first()
        .map(|point| point.timestamp())
        .unwrap_or(0);

    let mut options = historical_data.series_info.options.clone();
    options.replay_mode = true;
    options.replay_from = earliest_timestamp;

    tracing::debug!("Setting replay mode with timestamp: {}", earliest_timestamp);
    websocket.set_market(options).await?;

    Ok(())
}

/// Process remaining data in channels
async fn process_remaining_data(
    data_rx: &mut mpsc::Receiver<(SeriesInfo, Vec<DataPoint>)>,
    info_rx: &mut mpsc::Receiver<SymbolInfo>,
    historical_data: &mut ChartHistoricalData,
) {
    // Process remaining data points
    while let Ok(Some((series_info, data_points))) =
        timeout(REMAINING_DATA_TIMEOUT, data_rx.recv()).await
    {
        tracing::debug!(
            "Processing final batch of {} data points",
            data_points.len()
        );
        historical_data.series_info = series_info;
        historical_data.data.extend(data_points);
    }

    // Process remaining symbol info
    while let Ok(Some(symbol_info)) = timeout(REMAINING_DATA_TIMEOUT, info_rx.recv()).await {
        tracing::debug!("Processing final symbol info: {:?}", symbol_info);
        historical_data.symbol_info = symbol_info;
    }
}

/// Cleanup resources
async fn cleanup_resources(
    websocket: Arc<WebSocket>,
    subscription_handle: tokio::task::JoinHandle<()>,
) {
    if let Err(e) = websocket.delete().await {
        tracing::error!("Failed to close WebSocket connection: {}", e);
    }
    subscription_handle.abort();
}

/// Create batch callbacks with symbol counter
fn create_batch_callbacks(
    data_tx: Arc<Mutex<DataChannel>>,
    info_tx: Arc<Mutex<InfoChannel>>,
    completion_tx: Arc<Mutex<Option<CompletionChannel>>>,
    symbols_count: Arc<Mutex<usize>>,
) -> EventCallback {
    EventCallback::default()
        .on_chart_data(create_chart_data_handler(data_tx))
        .on_symbol_info(create_symbol_info_handler(info_tx))
        .on_series_completed(create_batch_completion_handler(
            completion_tx,
            symbols_count.clone(),
        ))
        .on_error(create_batch_error_handler(symbols_count))
}

/// Create batch completion handler
fn create_batch_completion_handler(
    completion_tx: Arc<Mutex<Option<CompletionChannel>>>,
    symbols_count: Arc<Mutex<usize>>,
) -> impl Fn(Vec<Value>) + Send + Sync + 'static {
    move |message: Vec<Value>| {
        let completion_tx = Arc::clone(&completion_tx);
        let symbols_count = Arc::clone(&symbols_count);
        tracing::info!("{}", serde_json::to_string(&message).unwrap_or_default());
        spawn(async move {
            if let Ok(mut count) = symbols_count.try_lock() {
                *count = count.saturating_sub(1);
                tracing::debug!("Series completed, remaining symbols: {}", *count);

                if *count == 0 {
                    send_completion_signal(completion_tx, "All symbols processed").await;
                }
            }
        });
    }
}

/// Create batch error handler
fn create_batch_error_handler(
    symbols_count: Arc<Mutex<usize>>,
) -> impl Fn((Error, Vec<Value>)) + Send + Sync + 'static {
    move |(error, message)| {
        tracing::error!("WebSocket error: {}", error);
        tracing::error!("{}", serde_json::to_string(&message).unwrap_or_default());
        let symbols_count = Arc::clone(&symbols_count);

        spawn(async move {
            if let Ok(mut count) = symbols_count.try_lock() {
                *count = count.saturating_sub(1);
                tracing::debug!("Error occurred, remaining symbols: {}", *count);
            }
        });
    }
}

/// Internal batch fetching implementation
async fn fetch_chart_data_batch_inner(
    auth_token: &str,
    symbols: &[impl MarketSymbol],
    base_options: ChartOptions,
    server: Option<DataServer>,
    batch_size: usize,
) -> Result<HashMap<String, ChartHistoricalData>> {
    let symbols_count = symbols.len();
    let (data_tx, data_rx) = mpsc::channel(DATA_CHANNEL_BUFFER);
    let (info_tx, info_rx) = mpsc::channel(INFO_CHANNEL_BUFFER);
    let (completion_tx, completion_rx) = oneshot::channel();

    let data_tx = Arc::new(Mutex::new(data_tx));
    let info_tx = Arc::new(Mutex::new(info_tx));
    let completion_tx = Arc::new(Mutex::new(Some(completion_tx)));
    let symbols_counter = Arc::new(Mutex::new(symbols_count));

    let callbacks = create_batch_callbacks(data_tx, info_tx, completion_tx, symbols_counter);

    let websocket = setup_batch_websocket_connection(
        auth_token,
        server,
        callbacks,
        symbols,
        base_options,
        batch_size,
    )
    .await?;

    let websocket = Arc::new(websocket);
    let subscription_handle = start_subscription_task(Arc::clone(&websocket));

    let results = collect_batch_results(data_rx, info_rx, completion_rx).await?;

    cleanup_resources(websocket, subscription_handle).await;

    tracing::debug!("Batch collection completed with {} symbols", results.len());
    Ok(results)
}

/// Collect results for batch processing
async fn collect_batch_results(
    mut data_rx: mpsc::Receiver<(SeriesInfo, Vec<DataPoint>)>,
    mut info_rx: mpsc::Receiver<SymbolInfo>,
    completion_rx: oneshot::Receiver<()>,
) -> Result<HashMap<String, ChartHistoricalData>> {
    let mut results = HashMap::new();
    let mut completion_future = Box::pin(completion_rx);

    loop {
        tokio::select! {
            Some((series_info, data_points)) = data_rx.recv() => {
                if !data_points.is_empty() {
                    // Include interval in the key to prevent overwriting
                    let symbol_key = format!("{}:{}:{}",
                        series_info.options.exchange,
                        series_info.options.symbol,
                        series_info.options.interval
                    );
                    let historical_data = results.entry(symbol_key)
                        .or_insert_with(ChartHistoricalData::new);

                    historical_data.series_info = series_info;
                    historical_data.data.extend(data_points);
                }
            }
            Some(symbol_info) = info_rx.recv() => {
                // For symbol_info, we might need to match it to existing entries
                // or create entries for all intervals
                let base_key = symbol_info.id.clone();

                // Find all entries that match this symbol and update them
                for (key, historical_data) in results.iter_mut() {
                    if key.starts_with(&base_key) {
                        historical_data.symbol_info = symbol_info.clone();
                    }
                }

                // If no matching entries found, create a new one
                if !results.keys().any(|k| k.starts_with(&base_key)) {
                    let historical_data = results.entry(base_key)
                        .or_insert_with(ChartHistoricalData::new);
                    historical_data.symbol_info = symbol_info;
                }
            }
            _ = &mut completion_future => {
                tracing::debug!("All symbols completed");
                break;
            }
            else => {
                tracing::debug!("All channels closed");
                break;
            }
        }
    }

    // Process any remaining data
    process_remaining_batch_data(&mut data_rx, &mut info_rx, &mut results).await;

    Ok(results)
}

/// Process remaining batch data
async fn process_remaining_batch_data(
    data_rx: &mut mpsc::Receiver<(SeriesInfo, Vec<DataPoint>)>,
    info_rx: &mut mpsc::Receiver<SymbolInfo>,
    results: &mut HashMap<String, ChartHistoricalData>,
) {
    while let Ok(Some((series_info, data_points))) =
        timeout(REMAINING_DATA_TIMEOUT, data_rx.recv()).await
    {
        if !data_points.is_empty() {
            // Include interval in the key to prevent overwriting
            let symbol_key = format!(
                "{}:{}:{}",
                series_info.options.exchange,
                series_info.options.symbol,
                series_info.options.interval
            );
            let historical_data = results
                .entry(symbol_key)
                .or_insert_with(ChartHistoricalData::new);
            historical_data.series_info = series_info;
            historical_data.data.extend(data_points);
        }
    }

    while let Ok(Some(symbol_info)) = timeout(REMAINING_DATA_TIMEOUT, info_rx.recv()).await {
        let base_key = symbol_info.id.clone();

        // Update all matching entries
        for (key, historical_data) in results.iter_mut() {
            if key.starts_with(&base_key) {
                historical_data.symbol_info = symbol_info.clone();
            }
        }

        // If no matching entries found, create a new one
        if !results.keys().any(|k| k.starts_with(&base_key)) {
            let historical_data = results
                .entry(base_key)
                .or_insert_with(ChartHistoricalData::new);
            historical_data.symbol_info = symbol_info;
        }
    }
}

/// Setup batch WebSocket connection
async fn setup_batch_websocket_connection(
    auth_token: &str,
    server: Option<DataServer>,
    callbacks: EventCallback,
    symbols: &[impl MarketSymbol],
    base_options: ChartOptions,
    batch_size: usize,
) -> Result<WebSocket> {
    let client = WebSocketClient::default().set_callbacks(callbacks);

    let websocket = WebSocket::new()
        .server(server.unwrap_or(DataServer::ProData))
        .auth_token(auth_token)
        .client(client)
        .build()
        .await?;

    // Prepare market options for all symbols
    let market_options: Vec<_> = symbols
        .iter()
        .map(|symbol| {
            let mut opt = base_options.clone();
            opt.symbol = symbol.symbol().to_string();
            opt.exchange = symbol.exchange().to_string();
            opt
        })
        .collect();

    // Process markets in batches
    let websocket_clone = websocket.clone();
    spawn(async move {
        for chunk in market_options.chunks(batch_size) {
            for opt in chunk {
                if let Err(e) = websocket_clone.set_market(opt.clone()).await {
                    tracing::error!("Failed to set market for {}: {}", opt.symbol, e);
                }
            }
            sleep(BATCH_PROCESSING_DELAY).await;
        }
        tracing::debug!("Finished setting all {} markets", market_options.len());
    });

    Ok(websocket)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Country, Interval, MarketType, StocksType, list_symbols};
    use std::sync::Once;

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

        let options = ChartOptions::new_with("VCB", "HOSE", Interval::OneDay).bar_count(10);

        let data = fetch_chart_data()
            .auth_token(&auth_token)
            .options(options)
            .server(DataServer::ProData)
            .call()
            .await?;

        assert!(!data.data.is_empty(), "Data should not be empty");
        assert_eq!(data.data.len(), 10, "Data length should be 10");
        assert_eq!(data.symbol_info.id, "HOSE:VCB", "Symbol should match");

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
        let symbols = &symbols[0..50];

        let auth_token = std::env::var("TV_AUTH_TOKEN").expect("TV_AUTH_TOKEN is not set");

        let data = fetch_chart_data_batch()
            .auth_token(&auth_token)
            .symbols(symbols)
            .interval(&[Interval::OneDay])
            .batch_size(40)
            .call()
            .await?;

        assert!(!data.is_empty(), "Should have collected some data");

        for (key, dataset) in data {
            println!("Symbol: {}, Data points: {}", key, dataset.data.len());
        }

        Ok(())
    }
}
