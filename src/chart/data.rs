use crate::{
    ChartHistoricalData, DataPoint, Result, Symbol, SymbolInfo,
    callback::EventCallback,
    chart::ChartOptions,
    socket::DataServer,
    websocket::{SeriesInfo, WebSocket, WebSocketClient},
};
use serde_json::Value;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::{
    spawn,
    sync::{Mutex, mpsc, oneshot},
    time::{sleep, timeout},
};

/// Fetch historical chart data from TradingView
pub async fn fetch_chart_historical(
    auth_token: &str,
    options: ChartOptions,
    server: Option<DataServer>,
) -> Result<ChartHistoricalData> {
    // Create communication channels with appropriate buffer sizes
    let (data_sender, mut data_receiver) = mpsc::channel::<(SeriesInfo, Vec<DataPoint>)>(100);
    let (info_sender, mut info_receiver) = mpsc::channel::<SymbolInfo>(100);
    let (completion_sender, completion_receiver) = oneshot::channel::<()>();

    // Wrap channels in thread-safe containers
    let data_sender = Arc::new(Mutex::new(data_sender));
    let completion_sender = Arc::new(Mutex::new(Some(completion_sender)));

    // Create callback handlers
    let callbacks = create_data_callbacks(
        Arc::clone(&data_sender),
        Arc::clone(&completion_sender),
        Arc::new(Mutex::new(info_sender)),
    );

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
    let result =
        collect_historical_data(&mut data_receiver, &mut info_receiver, completion_receiver).await;

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
#[allow(clippy::type_complexity)]
fn create_data_callbacks(
    data_sender: Arc<Mutex<mpsc::Sender<(SeriesInfo, Vec<DataPoint>)>>>,
    completion_sender: Arc<Mutex<Option<oneshot::Sender<()>>>>,
    info_sender: Arc<Mutex<mpsc::Sender<SymbolInfo>>>,
) -> EventCallback {
    EventCallback::default()
        .on_chart_data(
            move |(series_info, data_points): (SeriesInfo, Vec<DataPoint>)| {
                tracing::debug!("Received data batch with {} points", data_points.len());
                let sender = Arc::clone(&data_sender);
                spawn(async move {
                    let tx = sender.lock().await;
                    if let Err(e) = tx.send((series_info, data_points)).await {
                        tracing::error!("Failed to send data points to processing channel: {}", e);
                    }
                });
            },
        )
        .on_symbol_info(move |symbol_info| {
            tracing::debug!("Received symbol info: {:?}", symbol_info);
            let sender = Arc::clone(&info_sender);
            spawn(async move {
                let tx = sender.lock().await;
                if let Err(e) = tx.send(symbol_info).await {
                    tracing::error!("Failed to send symbol info to processing channel: {}", e);
                }
            });
        })
        .on_series_completed({
            let completion_sender = Arc::clone(&completion_sender);
            move |message: Vec<Value>| {
                let completion_sender = Arc::clone(&completion_sender);
                tracing::debug!("Series completed with message: {:?}", message);
                spawn(async move {
                    if let Some(sender) = completion_sender.lock().await.take() {
                        if let Err(e) = sender.send(()) {
                            tracing::error!("Failed to send completion signal: {:?}", e);
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
    data_receiver: &mut mpsc::Receiver<(SeriesInfo, Vec<DataPoint>)>,
    info_receiver: &mut mpsc::Receiver<SymbolInfo>,
    completion_receiver: oneshot::Receiver<()>,
) -> ChartHistoricalData {
    let mut historical_data = ChartHistoricalData::new();
    let mut completion_future = Box::pin(completion_receiver);

    // Process incoming data until completion signal or channel closure
    loop {
        tokio::select! {
            Some((series_info, data_points)) = data_receiver.recv() => {
                tracing::debug!("Processing batch of {} data points", data_points.len());
                historical_data.series_info = series_info;
                historical_data.data.extend(data_points);
            }
            _ = &mut completion_future => {
                tracing::debug!("Completion signal received");
                // Process any remaining data points with timeout
                process_remaining_data_points(data_receiver, &mut historical_data).await;
                process_remaining_symbol_info(info_receiver, &mut historical_data).await;
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

#[allow(clippy::type_complexity)]
fn create_batch_data_callbacks(
    data_sender: Arc<Mutex<mpsc::Sender<(SeriesInfo, Vec<DataPoint>)>>>,
    completion_sender: Arc<Mutex<Option<oneshot::Sender<()>>>>,
    info_sender: Arc<Mutex<mpsc::Sender<SymbolInfo>>>,
    symbols_count: Arc<Mutex<usize>>, // Add counter for tracking symbols
) -> EventCallback {
    EventCallback::default()
        .on_chart_data(
            move |(series_info, data_points): (SeriesInfo, Vec<DataPoint>)| {
                tracing::debug!("Received data batch with {} points", data_points.len());
                let sender = Arc::clone(&data_sender);
                spawn(async move {
                    let tx = sender.lock().await;
                    if let Err(e) = tx.send((series_info, data_points)).await {
                        tracing::error!("Failed to send data points to processing channel: {}", e);
                    }
                });
            },
        )
        .on_symbol_info(move |symbol_info| {
            tracing::debug!("Received symbol info: {:?}", symbol_info);
            let sender = Arc::clone(&info_sender);
            spawn(async move {
                let tx = sender.lock().await;
                if let Err(e) = tx.send(symbol_info).await {
                    tracing::error!("Failed to send symbol info to processing channel: {}", e);
                }
            });
        })
        .on_series_completed({
            let completion_sender = Arc::clone(&completion_sender);
            let symbols_count = Arc::clone(&symbols_count);
            move |message: Vec<Value>| {
                let completion_sender = Arc::clone(&completion_sender);
                let symbols_count = Arc::clone(&symbols_count);
                tracing::debug!("Series completed with message: {:?}", message);
                spawn(async move {
                    // Decrement the counter when a series completes
                    let mut count = symbols_count.lock().await;
                    *count -= 1;
                    tracing::debug!("Series completed, remaining symbols: {}", *count);

                    // If all symbols are processed, send completion signal
                    if *count == 0 {
                        tracing::debug!("All symbols processed, sending completion signal");
                        if let Some(sender) = completion_sender.lock().await.take() {
                            if let Err(e) = sender.send(()) {
                                tracing::error!("Failed to send completion signal: {:?}", e);
                            }
                        }
                    }
                });
            }
        })
}

async fn setup_batch_websocket_connection(
    auth_token: &str,
    server: Option<DataServer>,
    callbacks: EventCallback,
    symbols: &[Symbol],
    base_options: ChartOptions,
) -> Result<WebSocket> {
    let client = WebSocketClient::default().set_callbacks(callbacks);

    let websocket = WebSocket::new()
        .server(server.unwrap_or(DataServer::ProData))
        .auth_token(auth_token)
        .client(client)
        .build()
        .await?;

    // Prepare all market options
    let mut opts = Vec::new();
    for symbol in symbols {
        let mut opt = base_options.clone();
        opt.symbol = symbol.symbol.clone();
        opt.exchange = symbol.exchange.clone();
        opts.push(opt);
    }

    // Clone websocket for the background task
    let websocket_clone = websocket.clone();

    // Process market settings in background with batching
    tokio::spawn(async move {
        // Process in batches of 15
        let batch_size = 15;

        for chunk in opts.chunks(batch_size) {
            // Process one batch
            for opt in chunk {
                if let Err(e) = websocket_clone.set_market(opt.clone()).await {
                    tracing::error!("Failed to set market for {}: {}", opt.symbol, e);
                }
            }

            // Wait 5 seconds before processing the next batch
            sleep(Duration::from_secs(2)).await;
            tracing::debug!(
                "Processed batch of {} markets, waiting before next batch",
                chunk.len()
            );
        }

        tracing::debug!("Finished setting all {} markets", opts.len());
    });

    Ok(websocket)
}

pub async fn batch_fetch_chart_historical(
    auth_token: &str,
    symbols: &[Symbol],
    base_options: ChartOptions,
    server: Option<DataServer>,
) -> Result<HashMap<String, ChartHistoricalData>> {
    let symbols_count = symbols.len();
    // Create communication channels with appropriate buffer sizes
    let (data_sender, mut data_receiver) = mpsc::channel::<(SeriesInfo, Vec<DataPoint>)>(100);
    let (info_sender, mut info_receiver) = mpsc::channel::<SymbolInfo>(100);
    let (completion_sender, completion_receiver) = oneshot::channel::<()>();

    // Wrap channels in thread-safe containers
    let data_sender = Arc::new(Mutex::new(data_sender));
    let completion_sender = Arc::new(Mutex::new(Some(completion_sender)));
    let symbols_counter = Arc::new(Mutex::new(symbols_count));

    // Create callback handlers
    let callbacks = create_batch_data_callbacks(
        Arc::clone(&data_sender),
        Arc::clone(&completion_sender),
        Arc::new(Mutex::new(info_sender)),
        Arc::clone(&symbols_counter),
    );

    // Initialize and configure WebSocket connection
    let websocket = setup_batch_websocket_connection(
        auth_token,
        server,
        callbacks,
        symbols,
        base_options.clone(),
    )
    .await?;
    let websocket_shared = Arc::new(websocket);
    let websocket_shared_for_loop = Arc::clone(&websocket_shared);

    // Start the subscription process in a background task
    let subscription_task = spawn(async move {
        Arc::clone(&websocket_shared_for_loop).subscribe().await;
    });

    // Collect results for multiple symbols
    let mut results = HashMap::new();

    // Box and pin the completion receiver to prevent moves
    let mut completion_future = Box::pin(completion_receiver);

    // Process incoming data until completion signal
    loop {
        tokio::select! {
            Some((series_info, data_points)) = data_receiver.recv() => {
                let symbol_key = format!("{}:{}", series_info.options.exchange, series_info.options.symbol);
                tracing::debug!("Processing batch for symbol {}: {} points", symbol_key, data_points.len());

                // Get or create entry for this symbol
                let historical_data = results.entry(symbol_key.clone())
                    .or_insert_with(ChartHistoricalData::new);

                // Update data for this symbol
                historical_data.series_info = series_info;
                historical_data.data.extend(data_points);
            }
            Some(symbol_info) = info_receiver.recv() => {
                let symbol_key = symbol_info.id.clone();
                tracing::debug!("Processing symbol info for {}: {:?}", symbol_key, symbol_info);

                // Get or create entry for this symbol
                let historical_data = results.entry(symbol_key)
                    .or_insert_with(ChartHistoricalData::new);

                // Update symbol info
                historical_data.symbol_info = symbol_info;
            }
            _ = &mut completion_future => {
                tracing::debug!("All symbols completed, finishing data collection");
                break;
            }
            else => {
                tracing::debug!("All channels closed, no more data to receive");
                break;
            }
        }
    }

    // Process any remaining data with timeout
    while let Ok(Some((series_info, data_points))) =
        timeout(Duration::from_millis(100), data_receiver.recv()).await
    {
        let symbol_key = format!(
            "{}:{}",
            series_info.options.exchange, series_info.options.symbol
        );
        tracing::debug!(
            "Processing final batch for {}: {} points",
            symbol_key,
            data_points.len()
        );

        let historical_data = results
            .entry(symbol_key)
            .or_insert_with(ChartHistoricalData::new);

        historical_data.series_info = series_info;
        historical_data.data.extend(data_points);
    }

    // Process any remaining symbol info with timeout
    while let Ok(Some(symbol_info)) =
        timeout(Duration::from_millis(100), info_receiver.recv()).await
    {
        let symbol_key = symbol_info.id.clone();
        tracing::debug!("Processing final symbol info for {}", symbol_key);

        let historical_data = results
            .entry(symbol_key)
            .or_insert_with(ChartHistoricalData::new);

        historical_data.symbol_info = symbol_info;
    }

    // Cleanup resources
    if let Err(e) = websocket_shared.delete().await {
        tracing::error!("Failed to close WebSocket connection: {}", e);
    }
    subscription_task.abort();

    tracing::debug!("Data collection completed with {} symbols", results.len());

    Ok(results)
}

#[cfg(test)]
#[allow(unused)]
mod tests {
    use super::*;
    use crate::{Interval, MarketType, StocksType, chart::data, list_symbols};
    use anyhow::Ok;
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
        let symbol = "VCB";
        let exchange = "HOSE";
        let interval = Interval::Daily;
        let option = ChartOptions::new_with(symbol, exchange, interval).bar_count(10);
        let server = Some(DataServer::ProData);
        let data = fetch_chart_historical(&auth_token, option, server).await?;
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
            .country("VN")
            .call()
            .await?;
        assert!(!symbols.is_empty(), "No symbols found");
        let symbols = symbols[0..50].to_vec();

        println!("Fetched {} symbols", symbols.len());
        let auth_token = std::env::var("TV_AUTH_TOKEN").expect("TV_AUTH_TOKEN is not set");
        let based_opt = ChartOptions::builder()
            .interval(Interval::Daily)
            .bar_count(10)
            .build();

        let server = Some(DataServer::ProData);
        let data: HashMap<String, ChartHistoricalData> =
            batch_fetch_chart_historical(&auth_token, &symbols, based_opt, server).await?;

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
