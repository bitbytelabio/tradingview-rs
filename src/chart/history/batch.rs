use crate::{
    handler::event::TradingViewHandler, chart::ChartOptions, history::resolve_auth_token, socket::DataServer, websocket::{SeriesInfo, WebSocketClient, WebSocketHandler}, ChartHistoricalData, DataPoint, Error, Interval, MarketSymbol, Result, SymbolInfo, OHLCV as _
};
use bon::builder;
use dashmap::DashMap;
use serde_json::Value;
use ustr::Ustr;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::{
    spawn,
    sync::{Mutex, mpsc, oneshot},
    time::{sleep, timeout},
};

/// Tracks completion state and errors for batch processing
#[derive(Debug, Clone)]
struct BatchTracker {
    remaining: Arc<Mutex<usize>>,
    errors: Arc<Mutex<Vec<String>>>,
    completed: Arc<Mutex<Vec<String>>>,
    empty: Arc<Mutex<Vec<String>>>,
    // Track mapping between series ID and symbol
    series_map: Arc<Mutex<HashMap<String, String>>>,
    // Track completed series IDs
    completed_series: Arc<Mutex<Vec<String>>>,
}

impl BatchTracker {
    fn new(count: usize) -> Self {
        Self {
            remaining: Arc::new(Mutex::new(count)),
            errors: Arc::new(Mutex::new(Vec::new())),
            completed: Arc::new(Mutex::new(Vec::new())),
            empty: Arc::new(Mutex::new(Vec::new())),
            series_map: Arc::new(Mutex::new(HashMap::new())),
            completed_series: Arc::new(Mutex::new(Vec::new())),
        }
    }

    async fn register_series(&self, series_id: &str, symbol: &str) {
        let mut map = self.series_map.lock().await;
        map.insert(series_id.to_string(), symbol.to_string());
        tracing::debug!("Registered series {} for symbol {}", series_id, symbol);
    }

    async fn mark_completed(&self, series_id: &str, has_data: bool) -> bool {
        let mut done_series = self.completed_series.lock().await;
        let mut remaining = self.remaining.lock().await;
        let mut completed = self.completed.lock().await;
        let map = self.series_map.lock().await;

        // Check if already completed
        if done_series.contains(&series_id.to_string()) {
            tracing::debug!("Series {} already completed", series_id);
            return *remaining == 0;
        }

        done_series.push(series_id.to_string());

        // Get symbol for this series
        let symbol = map.get(series_id)
            .cloned()
            .unwrap_or_else(|| format!("unknown_{series_id}"));

        *remaining = remaining.saturating_sub(1);
        completed.push(symbol.clone());

        if !has_data {
            let mut empty = self.empty.lock().await;
            empty.push(symbol.clone());
            tracing::warn!("Series {} (symbol {}) completed with no data", series_id, symbol);
        }

        tracing::debug!("Series {} (symbol {}) completed, remaining: {}", series_id, symbol, *remaining);
        *remaining == 0
    }

    async fn add_error(&self, error: String) {
        let mut errors = self.errors.lock().await;
        let mut remaining = self.remaining.lock().await;

        errors.push(error.clone());
        *remaining = remaining.saturating_sub(1);

        tracing::error!("Error: {}, remaining: {}", error, *remaining);
    }

    async fn is_done(&self) -> bool {
        let remaining = self.remaining.lock().await;
        *remaining == 0
    }

    async fn summary(&self) -> (Vec<String>, Vec<String>, Vec<String>) {
        let errors = self.errors.lock().await.clone();
        let completed = self.completed.lock().await.clone();
        let empty = self.empty.lock().await.clone();
        (errors, completed, empty)
    }
}

#[allow(clippy::type_complexity)]
fn create_callbacks(
    data_tx: Arc<Mutex<mpsc::Sender<(SeriesInfo, Vec<DataPoint>)>>>,
    done_tx: Arc<Mutex<Option<oneshot::Sender<()>>>>,
    info_tx: Arc<Mutex<mpsc::Sender<SymbolInfo>>>,
    tracker: BatchTracker,
    data_map: Arc<DashMap<Ustr, bool>>, // Track which series received data
) -> TradingViewHandler {
    TradingViewHandler::default()
        .on_chart_data({
            let tracker = tracker.clone();
            let data_map = Arc::clone(&data_map);
            move |(info, points): (SeriesInfo, Vec<DataPoint>)| {
                let data_tx = Arc::clone(&data_tx);
                let tracker = tracker.clone();
                let data_map = Arc::clone(&data_map);
                
                spawn(async move {
                    let symbol = format!("{}:{}", info.options.exchange, info.options.symbol);
                    let series_id = info.chart_session;
                    
                    // Register series to symbol mapping
                    tracker.register_series(&series_id, &symbol).await;
                    
                    // Track data received
                    
                    data_map.insert(series_id, !points.is_empty());
                    
                    
                    if points.is_empty() {
                        tracing::warn!("Empty data for series {} (symbol {})", series_id, symbol);
                    } else {
                        tracing::debug!("Received {} points for series {} (symbol {})", 
                                      points.len(), series_id, symbol);
                    }
                    
                    let tx = data_tx.lock().await;
                    if let Err(e) = tx.send((info, points)).await {
                        let error = format!("Failed to send data for series {series_id} (symbol {symbol}): {e}");
                        tracker.add_error(error).await;
                    }
                });
            }
        })
        .on_symbol_info({
            let tracker = tracker.clone();
            move |info| {
                let info_tx = Arc::clone(&info_tx);
                let tracker = tracker.clone();
                
                spawn(async move {
                    tracing::debug!("Received symbol info: {:?}", info);
                    let tx = info_tx.lock().await;
                    if let Err(e) = tx.send(info.clone()).await {
                        let error = format!("Failed to send symbol info for {}: {}", info.id, e);
                        tracker.add_error(error).await;
                    }
                });
            }
        })
        .on_series_completed({
            let done_tx = Arc::clone(&done_tx);
            let tracker = tracker.clone();
            let data_map = Arc::clone(&data_map);
            
            move |msg: Vec<Value>| {
                let done_tx = Arc::clone(&done_tx);
                let tracker = tracker.clone();
                let data_map = Arc::clone(&data_map);
                
                spawn(async move {
                    tracing::debug!("Series completed: {:?}", msg);
                    
                    // Extract series ID from completion message
                    let series_id = extract_series_id(&msg)
                        .unwrap_or_else(|| "unknown".into());
                    
                    tracing::debug!("Extracted series ID: {}", series_id);
                    
                    // Check if series has data
                    let has_data = {
                        data_map.get(&series_id).is_some_and(|v| *v)
                    };
                    
                    // Mark as completed and check if all done
                    let all_done = tracker.mark_completed(&series_id, has_data).await;
                    
                    if all_done {
                        let (errors, completed, empty) = tracker.summary().await;
                        
                        tracing::info!(
                            "All done - Completed: {}, Empty: {}, Errors: {}", 
                            completed.len(), 
                            empty.len(), 
                            errors.len()
                        );
                        
                        if !empty.is_empty() {
                            tracing::warn!("Empty symbols: {:?}", empty);
                        }
                        
                        if !errors.is_empty() {
                            tracing::error!("Errors: {:?}", errors);
                        }
                        
                        if let Some(tx) = done_tx.lock().await.take() {
                            if tx.send(()).is_err() {
                                tracing::error!("Failed to send completion signal");
                            }
                        }
                    }
                });
            }
        })
        .on_error({
            let tracker = tracker.clone();
            move |error| {
                let tracker = tracker.clone();
                spawn(async move {
                    let error_msg = format!("WebSocket error: {error:?}");
                    tracker.add_error(error_msg).await;
                    
                    if tracker.is_done().await {
                        tracing::error!("All symbols processed due to errors");
                    }
                });
            }
        })
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
                return Some(s.into());
            }
        }
    }
    
    tracing::warn!("No series ID found in: {:?}", msg);
    None
}

async fn setup_websocket(
    auth_token: &str,
    server: Option<DataServer>,
    callbacks: TradingViewHandler,
    symbols: &[impl MarketSymbol],
    interval: Interval,
    batch_size: usize,
) -> Result<WebSocketClient> {
    let client = WebSocketHandler::default().set_handler(callbacks);

    let ws = WebSocketClient::builder()
        .server(server.unwrap_or(DataServer::ProData))
        .auth_token(auth_token)
        .handler(client)
        .build()
        .await?;

    // Prepare market options
    let mut opts = Vec::new();
    for symbol in symbols {
        let opt = ChartOptions::new_with(symbol.symbol(), symbol.exchange(), interval);
        opts.push(opt);
    }

    let ws_clone = ws.clone();

    // Set markets in batches
    spawn(async move {
        for (idx, chunk) in opts.chunks(batch_size).enumerate() {
            tracing::debug!("Processing batch {} with {} symbols", idx + 1, chunk.len());
            
            for opt in chunk {
                match ws_clone.set_market(*opt).await {
                    Ok(_) => {
                        tracing::debug!("Set market: {}:{}", opt.exchange, opt.symbol);
                    }
                    Err(e) => {
                        tracing::error!("Failed to set market {}:{}: {}", opt.exchange, opt.symbol, e);
                    }
                }
            }

            // Wait between batches (except last)
            if idx < opts.len().div_ceil(batch_size) - 1 {
                sleep(Duration::from_secs(5)).await;
                tracing::debug!("Batch {} done, waiting", idx + 1);
            }
        }

        tracing::debug!("All {} markets set", opts.len());
    });

    Ok(ws)
}

#[builder]
pub async fn retrieve(
    auth_token: Option<&str>,
    symbols: &[impl MarketSymbol],
    interval: Interval,
    server: Option<DataServer>,
    #[builder(default = 40)] batch_size: usize,
) -> Result<HashMap<String, (SymbolInfo, Interval, Vec<DataPoint>)>> {
    if symbols.is_empty() {
        return Err(Error::Generic(Ustr::from("No symbols provided for batch retrieval")));
    }

    let auth_token = resolve_auth_token(auth_token)?;
    let count = symbols.len();
    
    tracing::info!("Starting batch for {} symbols with batch size {}", count, batch_size);

    // Create channels
    let (data_tx, mut data_rx) = mpsc::channel::<(SeriesInfo, Vec<DataPoint>)>(100);
    let (info_tx, mut info_rx) = mpsc::channel::<SymbolInfo>(100);
    let (done_tx, done_rx) = oneshot::channel::<()>();

    // Initialize tracking
    let tracker = BatchTracker::new(count);
    let data_map = Arc::new(DashMap::new());

    // Wrap channels for sharing
    let data_tx = Arc::new(Mutex::new(data_tx));
    let done_tx = Arc::new(Mutex::new(Some(done_tx)));

    // Create callbacks
    let callbacks = create_callbacks(
        Arc::clone(&data_tx),
        Arc::clone(&done_tx),
        Arc::new(Mutex::new(info_tx)),
        tracker.clone(),
        Arc::clone(&data_map),
    );

    // Setup WebSocket
    let ws = setup_websocket(
        &auth_token,
        server,
        callbacks,
        symbols,
        interval,
        batch_size,
    )
    .await?;
    let ws = Arc::new(ws);
    let ws_sub = Arc::clone(&ws);

    // Start subscription
    let sub_task = spawn(async move {
       Arc::clone(&ws_sub).subscribe().await
    });

    // Collect results
    let mut results = HashMap::new();
    let mut done_future = Box::pin(done_rx);

    // Main processing loop with timeout
    let timeout_duration = Duration::from_secs(300); // 5 minutes
    let result = timeout(timeout_duration, async {
        loop {
            tokio::select! {
                Some((info, points)) = data_rx.recv() => {
                    let symbol = format!("{}:{}", info.options.exchange, info.options.symbol);
                    
                    if points.is_empty() {
                        tracing::warn!("Empty data for series {} (symbol {})", info.chart_session, symbol);
                    } else {
                        tracing::debug!("Processing {} points for series {} (symbol {})", 
                                      points.len(), info.chart_session, symbol);
                    }

                    let data = results.entry(symbol.clone())
                        .or_insert_with(ChartHistoricalData::new);

                    data.series_info = info;
                    data.data.extend(points);
                }
                Some(info) = info_rx.recv() => {
                    let symbol = info.id;
                    tracing::debug!("Processing symbol info for {}: {:?}", symbol, info);

                    let data = results.entry(symbol.to_string())
                        .or_insert_with(ChartHistoricalData::new);

                    data.symbol_info = info;
                }
                _ = &mut done_future => {
                    tracing::debug!("All symbols completed");
                    break;
                }
                else => {
                    tracing::debug!("All channels closed");
                    break;
                }
            }
        }
    }).await;

    match result {
        Ok(_) => {
            tracing::debug!("Collection completed");
        }
        Err(_) => {
            tracing::error!("Timed out after {} seconds", timeout_duration.as_secs());
            return Err(Error::Timeout(
                Ustr::from("Batch retrieval timed out after 5 minutes")
            ));
        }
    }

    // Process remaining data
    while let Ok(Some((info, points))) =
        timeout(Duration::from_millis(100), data_rx.recv()).await
    {
        let symbol = format!("{}:{}", info.options.exchange, info.options.symbol);
        tracing::debug!("Final batch for series {} (symbol {}): {} points",
                      info.chart_session, symbol, points.len());

        let data = results.entry(symbol)
            .or_insert_with(ChartHistoricalData::new);

        data.series_info = info;
        data.data.extend(points);
    }

    // Process remaining symbol info
    while let Ok(Some(info)) =
        timeout(Duration::from_millis(100), info_rx.recv()).await
    {
        let symbol = info.id;
        tracing::debug!("Final symbol info for {}", symbol);

        let data = results.entry(symbol.to_string())
            .or_insert_with(ChartHistoricalData::new);

        data.symbol_info = info;
    }

    // Get summary
    let (errors, completed, empty) = tracker.summary().await;

    // Cleanup
    if let Err(e) = ws.delete().await {
        tracing::error!("Failed to close WebSocket: {}", e);
    }
    sub_task.abort();

    tracing::info!(
        "Batch completed - Retrieved: {}, Completed: {}, Empty: {}, Errors: {}", 
        results.len(), 
        completed.len(), 
        empty.len(), 
        errors.len()
    );

    if !errors.is_empty() {
        tracing::warn!("Errors occurred: {:?}", errors);
    }

    let mut final_results = HashMap::new();
    for (symbol, mut result) in results {
        tracing::debug!("Symbol: {}, Data points: {}", symbol, result.data.len());
        let mut data = std::mem::take(&mut result.data);
        data.dedup_by_key(|point| point.timestamp());
        data.sort_by_key(|a| a.timestamp());
        final_results.insert(symbol.clone(), (result.symbol_info.clone(), result.series_info.options.interval, data));
    }

    Ok(final_results)
}