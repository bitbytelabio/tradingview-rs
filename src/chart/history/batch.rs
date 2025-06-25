use crate::{
    callback::EventCallback, chart::ChartOptions, history::resolve_auth_token, socket::DataServer, websocket::{SeriesInfo, WebSocket, WebSocketClient}, ChartHistoricalData, DataPoint, Error, Interval, MarketSymbol, Result, SymbolInfo
};
use bon::builder;
use serde_json::Value;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::{
    spawn,
    sync::{Mutex, mpsc, oneshot},
    time::{sleep, timeout},
};

/// Tracks completion state and errors for batch processing
#[derive(Debug, Clone)]
struct BatchState {
    symbols_remaining: Arc<Mutex<usize>>,
    errors: Arc<Mutex<Vec<String>>>,
    completed_symbols: Arc<Mutex<Vec<String>>>,
    empty_data_symbols: Arc<Mutex<Vec<String>>>,
    // Track mapping between chart series ID and symbol
    series_id_to_symbol: Arc<Mutex<HashMap<String, String>>>,
    // Track which series have been completed
    completed_series_ids: Arc<Mutex<Vec<String>>>,
}

impl BatchState {
    fn new(symbol_count: usize) -> Self {
        Self {
            symbols_remaining: Arc::new(Mutex::new(symbol_count)),
            errors: Arc::new(Mutex::new(Vec::new())),
            completed_symbols: Arc::new(Mutex::new(Vec::new())),
            empty_data_symbols: Arc::new(Mutex::new(Vec::new())),
            series_id_to_symbol: Arc::new(Mutex::new(HashMap::new())),
            completed_series_ids: Arc::new(Mutex::new(Vec::new())),
        }
    }

    async fn register_series_id(&self, series_id: &str, symbol_key: &str) {
        let mut mapping = self.series_id_to_symbol.lock().await;
        mapping.insert(series_id.to_string(), symbol_key.to_string());
        tracing::debug!("Registered series ID {} for symbol {}", series_id, symbol_key);
    }

    async fn mark_series_completed(&self, series_id: &str, has_data: bool) -> bool {
        let mut completed_series = self.completed_series_ids.lock().await;
        let mut remaining = self.symbols_remaining.lock().await;
        let mut completed = self.completed_symbols.lock().await;
        let mapping = self.series_id_to_symbol.lock().await;

        // Check if this series was already completed
        if completed_series.contains(&series_id.to_string()) {
            tracing::debug!("Series {} already marked as completed", series_id);
            return *remaining == 0;
        }

        completed_series.push(series_id.to_string());

        // Get the symbol key for this series ID
        let symbol_key = mapping.get(series_id)
            .cloned()
            .unwrap_or_else(|| format!("unknown_series_{}", series_id));

        *remaining = remaining.saturating_sub(1);
        completed.push(symbol_key.clone());

        if !has_data {
            let mut empty_symbols = self.empty_data_symbols.lock().await;
            empty_symbols.push(symbol_key.clone());
            tracing::warn!("Series {} (symbol {}) completed with no data", series_id, symbol_key);
        }

        tracing::debug!("Series {} (symbol {}) completed, remaining: {}", series_id, symbol_key, *remaining);
        *remaining == 0
    }

    async fn add_error(&self, error: String) {
        let mut errors = self.errors.lock().await;
        let mut remaining = self.symbols_remaining.lock().await;

        errors.push(error.clone());
        *remaining = remaining.saturating_sub(1);

        tracing::error!(
            "Error recorded: {}, remaining symbols: {}",
            error,
            *remaining
        );
    }

    async fn is_complete(&self) -> bool {
        let remaining = self.symbols_remaining.lock().await;
        *remaining == 0
    }

    async fn get_summary(&self) -> (Vec<String>, Vec<String>, Vec<String>) {
        let errors = self.errors.lock().await.clone();
        let completed = self.completed_symbols.lock().await.clone();
        let empty_data = self.empty_data_symbols.lock().await.clone();
        (errors, completed, empty_data)
    }
}

#[allow(clippy::type_complexity)]
fn create_batch_data_callbacks(
    data_sender: Arc<Mutex<mpsc::Sender<(SeriesInfo, Vec<DataPoint>)>>>,
    completion_sender: Arc<Mutex<Option<oneshot::Sender<()>>>>,
    info_sender: Arc<Mutex<mpsc::Sender<SymbolInfo>>>,
    batch_state: BatchState,
    symbol_data_tracker: Arc<Mutex<HashMap<String, bool>>>, // Track which symbols have received data
) -> EventCallback {
    EventCallback::default()
        .on_chart_data({
            let batch_state = batch_state.clone();
            let symbol_data_tracker = Arc::clone(&symbol_data_tracker);
            move |(series_info, data_points): (SeriesInfo, Vec<DataPoint>)| {
                let data_sender = Arc::clone(&data_sender);
                let batch_state = batch_state.clone();
                let symbol_data_tracker = Arc::clone(&symbol_data_tracker);
                
                spawn(async move {
                    let symbol_key = format!("{}:{}", series_info.options.exchange, series_info.options.symbol);
                    let series_id = &series_info.chart_session.clone();
                    
                    // Register the series ID to symbol mapping
                    batch_state.register_series_id(series_id, &symbol_key).await;
                    
                    // Mark that this series has received data
                    {
                        let mut tracker = symbol_data_tracker.lock().await;
                        tracker.insert(series_id.clone(), !data_points.is_empty());
                    }
                    
                    if data_points.is_empty() {
                        tracing::warn!("Received empty data batch for series {} (symbol {})", series_id, symbol_key);
                    } else {
                        tracing::debug!("Received data batch with {} points for series {} (symbol {})", 
                                      data_points.len(), series_id, symbol_key);
                    }
                    
                    let tx = data_sender.lock().await;
                    if let Err(e) = tx.send((series_info, data_points)).await {
                        let error_msg = format!("Failed to send data points for series {} (symbol {}): {}", 
                                               series_id, symbol_key, e);
                        batch_state.add_error(error_msg).await;
                    }
                });
            }
        })
        .on_symbol_info({
            let batch_state = batch_state.clone();
            move |symbol_info| {
                let info_sender = Arc::clone(&info_sender);
                let batch_state = batch_state.clone();
                
                spawn(async move {
                    tracing::debug!("Received symbol info: {:?}", symbol_info);
                    let tx = info_sender.lock().await;
                    if let Err(e) = tx.send(symbol_info.clone()).await {
                        let error_msg = format!("Failed to send symbol info for {}: {}", symbol_info.id, e);
                        batch_state.add_error(error_msg).await;
                    }
                });
            }
        })
        .on_series_completed({
            let completion_sender = Arc::clone(&completion_sender);
            let batch_state = batch_state.clone();
            let symbol_data_tracker = Arc::clone(&symbol_data_tracker);
            
            move |message: Vec<Value>| {
                let completion_sender = Arc::clone(&completion_sender);
                let batch_state = batch_state.clone();
                let symbol_data_tracker = Arc::clone(&symbol_data_tracker);
                
                spawn(async move {
                    tracing::debug!("Series completed with message: {:?}", message);
                    
                    // Extract series ID from completion message
                    let series_id = get_chart_session_from_completion(&message)
                        .unwrap_or_else(|| "unknown_series".to_string());
                    
                    tracing::debug!("Extracted series ID from completion: {}", series_id);
                    
                    // Check if this series received any data
                    let has_data = {
                        let tracker = symbol_data_tracker.lock().await;
                        tracker.get(&series_id).copied().unwrap_or(false)
                    };
                    
                    // Mark series as completed and check if all are done
                    let all_complete = batch_state.mark_series_completed(&series_id, has_data).await;
                    
                    if all_complete {
                        let (errors, completed, empty_data) = batch_state.get_summary().await;
                        
                        tracing::info!(
                            "All symbols processed - Completed: {}, Empty data: {}, Errors: {}", 
                            completed.len(), 
                            empty_data.len(), 
                            errors.len()
                        );
                        
                        if !empty_data.is_empty() {
                            tracing::warn!("Symbols with no data: {:?}", empty_data);
                        }
                        
                        if !errors.is_empty() {
                            tracing::error!("Errors encountered: {:?}", errors);
                        }
                        
                        if let Some(sender) = completion_sender.lock().await.take() {
                            if let Err(_) = sender.send(()) {
                                tracing::error!("Failed to send completion signal - receiver may have been dropped");
                            }
                        }
                    }
                });
            }
        })
        .on_error({
            let batch_state = batch_state.clone();
            move |error| {
                let batch_state = batch_state.clone();
                spawn(async move {
                    let error_msg = format!("WebSocket error: {:?}", error);
                    batch_state.add_error(error_msg).await;
                    
                    // Check if this error caused all processing to complete
                    if batch_state.is_complete().await {
                        tracing::error!("All symbols processed due to errors");
                    }
                });
            }
        })
}

/// Extract series ID from completion message (looking for cs_tKeYG4jmczi0 pattern)
fn get_chart_session_from_completion(message: &[Value]) -> Option<String> {
    if message.is_empty() {
        tracing::warn!("Completion message is empty, cannot extract series ID");
        return None;
    }

    // Look for series ID in the completion message
    for value in message {  
        // If the value is a string and looks like a series ID
        if let Some(str_val) = value.as_str() {
            if str_val.starts_with("cs_") {
                return Some(str_val.to_string());
            }
        }
    }
    
    tracing::warn!("Could not extract series ID from completion message: {:?}", message);
    None
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
        for (batch_idx, chunk) in opts.chunks(batch_size).enumerate() {
            tracing::debug!("Processing batch {} with {} symbols", batch_idx + 1, chunk.len());
            
            // Process one batch
            for opt in chunk {
                match websocket_clone.set_market(opt.clone()).await {
                    Ok(_) => {
                        tracing::debug!("Successfully set market for {}:{}", opt.exchange, opt.symbol);
                    }
                    Err(e) => {
                        tracing::error!("Failed to set market for {}:{}: {}", opt.exchange, opt.symbol, e);
                    }
                }
            }

            // Wait 5 seconds before processing the next batch (except for the last batch)
            if batch_idx < (opts.len() + batch_size - 1) / batch_size - 1 {
                sleep(Duration::from_secs(5)).await;
                tracing::debug!("Processed batch {}, waiting before next batch", batch_idx + 1);
            }
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
    if symbols.is_empty() {
        return Err(Error::Generic("No symbols provided".to_string()));
    }

    let auth_token = resolve_auth_token(auth_token)?;
    let symbols_count = symbols.len();
    
    tracing::info!("Starting batch retrieval for {} symbols with batch size {}", symbols_count, batch_size);

    // Create communication channels with appropriate buffer sizes
    let (data_sender, mut data_receiver) = mpsc::channel::<(SeriesInfo, Vec<DataPoint>)>(100);
    let (info_sender, mut info_receiver) = mpsc::channel::<SymbolInfo>(100);
    let (completion_sender, completion_receiver) = oneshot::channel::<()>();

    // Initialize batch state and data tracking
    let batch_state = BatchState::new(symbols_count);
    let symbol_data_tracker = Arc::new(Mutex::new(HashMap::new()));

    // Wrap channels in thread-safe containers
    let data_sender = Arc::new(Mutex::new(data_sender));
    let completion_sender = Arc::new(Mutex::new(Some(completion_sender)));

    // Create callback handlers
    let callbacks = create_batch_data_callbacks(
        Arc::clone(&data_sender),
        Arc::clone(&completion_sender),
        Arc::new(Mutex::new(info_sender)),
        batch_state.clone(),
        Arc::clone(&symbol_data_tracker),
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
        Arc::clone(&websocket_shared_for_loop).subscribe().await
    });

    // Collect results for multiple symbols
    let mut results = HashMap::new();
    let mut completion_future = Box::pin(completion_receiver);

    // Add timeout for the entire operation
    let operation_timeout = Duration::from_secs(300); // 5 minutes
    let operation_result = timeout(operation_timeout, async {
        // Process incoming data until completion signal
        loop {
            tokio::select! {
                Some((series_info, data_points)) = data_receiver.recv() => {
                    let symbol_key = format!("{}:{}", series_info.options.exchange, series_info.options.symbol);
                    
                    if data_points.is_empty() {
                        tracing::warn!("Received empty data batch for series {} (symbol {})", series_info.chart_session, symbol_key);
                    } else {
                        tracing::debug!("Processing batch for series {} (symbol {}): {} points", 
                                      series_info.chart_session, symbol_key, data_points.len());
                    }

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
    }).await;

    match operation_result {
        Ok(_) => {
            tracing::debug!("Data collection completed successfully");
        }
        Err(_) => {
            tracing::error!("Data collection timed out after {} seconds", operation_timeout.as_secs());
            return Err(Error::TimeoutError("Batch operation timed out".to_string()));
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
            "Processing final batch for series {} (symbol {}): {} points",
            series_info.chart_session, symbol_key, data_points.len()
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

    // Get final batch state summary
    let (errors, completed_symbols, empty_data_symbols) = batch_state.get_summary().await;

    // Cleanup resources
    if let Err(e) = websocket_shared.delete().await {
        tracing::error!("Failed to close WebSocket connection: {}", e);
    }
    subscription_task.abort();

    tracing::info!(
        "Data collection completed - Retrieved: {}, Completed: {}, Empty: {}, Errors: {}", 
        results.len(), 
        completed_symbols.len(), 
        empty_data_symbols.len(), 
        errors.len()
    );

    if !errors.is_empty() {
        tracing::warn!("Some errors occurred during batch processing: {:?}", errors);
    }

    Ok(results)
}