use crate::{
    ChartHistoricalData, DataPoint, Interval, MarketSymbol, Result, SymbolInfo,
    callback::EventCallback,
    chart::ChartOptions,
    history::resolve_auth_token,
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
        .on_error({
            let symbols_count = Arc::clone(&symbols_count);
            move |error| {
                tracing::error!("WebSocket error: {:?}", error);
                let symbols_count = Arc::clone(&symbols_count);

                // Decrement the counter on error as well
                spawn(async move {
                    let mut count = symbols_count.lock().await;
                    *count -= 1;
                    tracing::debug!("Error occurred, remaining symbols: {}", *count);
                });
            }
        })
}

async fn setup_batch_websocket_connection(
    auth_token: &str,
    server: Option<DataServer>,
    callbacks: EventCallback,
    symbols: &[impl MarketSymbol],
    interval: Interval,
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
    let mut opts = Vec::new();
    for symbol in symbols {
        let opt = ChartOptions::new_with(symbol.symbol(), symbol.exchange(), interval);
        opts.push(opt);
    }

    // Clone websocket for the background task
    let websocket_clone = websocket.clone();

    // Process market settings in background with batching
    spawn(async move {
        for chunk in opts.chunks(batch_size) {
            // Process one batch
            for opt in chunk {
                if let Err(e) = websocket_clone.set_market(opt.clone()).await {
                    tracing::error!("Failed to set market for {}: {}", opt.symbol, e);
                }
            }

            // Wait 5 seconds before processing the next batch
            sleep(Duration::from_secs(5)).await;
            tracing::debug!(
                "Processed batch of {} markets, waiting before next batch",
                chunk.len()
            );
        }

        tracing::debug!("Finished setting all {} markets", opts.len());
    });

    Ok(websocket)
}

#[builder]
pub async fn retrieve(
    auth_token: Option<&str>,
    symbols: &[impl MarketSymbol],
    interval: Interval,
    server: Option<DataServer>,
    #[builder(default = 40)] batch_size: usize,
) -> Result<HashMap<String, ChartHistoricalData>> {
    let auth_token = resolve_auth_token(auth_token)?;

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
        &auth_token,
        server,
        callbacks,
        symbols,
        interval,
        batch_size,
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
