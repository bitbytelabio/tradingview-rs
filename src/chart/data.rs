use crate::{
    ChartHistoricalData, DataPoint, Result, Symbol, SymbolInfo,
    callback::Callbacks,
    chart::ChartOptions,
    socket::DataServer,
    websocket::{WebSocket, WebSocketClient},
};
use serde_json::Value;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::{
    spawn,
    sync::{Mutex, mpsc, oneshot},
    time::timeout,
};

/// Fetch historical chart data from TradingView
pub async fn fetch_chart_historical(
    auth_token: &str,
    options: ChartOptions,
    server: Option<DataServer>,
) -> Result<ChartHistoricalData> {
    // Use a bounded channel with sufficient capacity to provide backpressure if needed
    let (bar_tx, mut bar_rx) = mpsc::channel::<Vec<DataPoint>>(100);
    let (info_tx, mut info_rx) = mpsc::channel::<SymbolInfo>(100);
    // One-shot channel for series completion signal
    let (done_tx, done_rx) = oneshot::channel::<()>();

    // Create callback functions using Arc<Mutex> to ensure thread-safe access
    let bar_tx = Arc::new(Mutex::new(bar_tx));
    let done_tx = Arc::new(Mutex::new(Some(done_tx)));
    // Clone for callbacks to move into closures
    let bar_tx_clone = Arc::clone(&bar_tx);
    let info_tx_clone = Arc::new(Mutex::new(info_tx));

    let callbacks = Callbacks::default()
        .on_chart_data(move |(_, data_points): (ChartOptions, Vec<DataPoint>)| {
            tracing::debug!("Received data batch with {} points", data_points.len());
            let sender = bar_tx_clone.clone();
            // Spawn the async work and return ()
            spawn(async move {
                // Acquire mutex lock to send data points
                let tx = sender.lock().await;
                if let Err(e) = tx.send(data_points).await {
                    tracing::error!("Failed to send data points: {}", e);
                }
            });
        })
        .on_symbol_info(move |symbol_info| {
            tracing::debug!("Received symbol info: {:?}", symbol_info);
            let sender = info_tx_clone.clone();
            // Spawn the async work and return ()
            spawn(async move {
                // Acquire mutex lock to send symbol info
                let tx = sender.lock().await;
                if let Err(e) = tx.send(symbol_info).await {
                    tracing::error!("Failed to send symbol info: {}", e);
                }
            });
        })
        .on_series_completed({
            // Create a clone for the closure
            let done_sender = Arc::clone(&done_tx);
            move |message: Vec<Value>| {
                let done_sender = Arc::clone(&done_sender);
                tracing::debug!("Series completed with message: {:?}", message);
                spawn(async move {
                    // Take the sender out of the Option and send the signal
                    if let Some(sender) = done_sender.lock().await.take() {
                        let _ = sender.send(());
                    }
                });
            }
        });

    // --------------------------------- WebSocket -------------------------------- //
    let client = WebSocketClient::default().set_callbacks(callbacks);

    let websocket = WebSocket::new()
        .server(server.unwrap_or(DataServer::ProData))
        .auth_token(auth_token)
        .client(client)
        .build()
        .await?;

    // Create an Arc to safely share the WebSocket between tasks
    let websocket = Arc::new(websocket);
    let websocket_for_loop = Arc::clone(&websocket);

    // Set the market before spawning the task
    websocket.set_market(options.clone()).await?;
    // Spawn WebSocket subscription loop in background
    let subscription_handle = spawn(async move {
        websocket_for_loop.subscribe().await;
    });

    // --------------------------------- Accumulate -------------------------------- //
    let mut result = ChartHistoricalData::new(&options);

    // Create a future that completes when done_rx receives a value
    let mut completion_future = Box::pin(done_rx);

    // Use a loop with tokio::select! that doesn't consume the completion future
    loop {
        tokio::select! {
            Some(points) = bar_rx.recv() => {
                tracing::debug!("Processing batch of {} data points", points.len());
                result.data.extend(points);
            }
            _ = &mut completion_future => {
                tracing::debug!("Completion signal received");
                // Process any remaining data points
                while let Ok(Some(points)) = timeout(
                   Duration::from_millis(100),
                    bar_rx.recv()
                ).await {
                    tracing::debug!("Processing final batch of {} data points", points.len());
                    result.data.extend(points);
                }
                break;
            }
            Some(info) = info_rx.recv() => {
                tracing::debug!("Received symbol info: {:?}", info);
                result.info = info;
            }
            else => {
                tracing::debug!("No more data to receive");
                break;
            }
        }
    }

    // Ensure the WebSocket is properly closed
    if let Err(e) = websocket.delete().await {
        tracing::error!("Failed to close WebSocket: {}", e);
    }
    subscription_handle.abort();

    tracing::debug!(
        "Data collection completed with {} points",
        result.data.len()
    );
    Ok(result)
}

pub async fn batch_fetch_chart_historical(
    auth_token: &str,
    symbols: &[Symbol],
    based_opt: ChartOptions,
    server: Option<DataServer>,
) -> Result<HashMap<String, ChartHistoricalData>> {
    // Use a bounded channel with sufficient capacity to provide backpressure if needed
    let (bar_tx, mut bar_rx) = mpsc::channel::<(ChartOptions, Vec<DataPoint>)>(100);
    let (info_tx, mut info_rx) = mpsc::channel::<SymbolInfo>(100);

    // One-shot channel for series completion signal
    let (done_tx, done_rx) = oneshot::channel::<()>();

    // Create callback functions using Arc<Mutex> to ensure thread-safe access
    let bar_tx = Arc::new(Mutex::new(bar_tx));
    let done_tx = Arc::new(Mutex::new(Some(done_tx)));
    // Clone for callbacks to move into closures
    let bar_tx_clone = Arc::clone(&bar_tx);
    let info_tx_clone = Arc::new(Mutex::new(info_tx));
    let opts = symbols
        .iter()
        .map(|s| {
            let opt = ChartOptions::new_with(&s.symbol, &s.exchange, based_opt.interval)
                .bar_count(based_opt.bar_count)
                .replay_mode(based_opt.replay_mode)
                .replay_from(based_opt.replay_from)
                .replay_session_id(&based_opt.replay_session.clone().unwrap_or_default());
            if let Some(study) = &based_opt.study_config {
                opt.study_config(
                    &study.script_id,
                    &study.script_version,
                    study.script_type.clone(),
                )
            } else {
                opt
            }
        })
        .collect::<Vec<ChartOptions>>();
    let count = Arc::new(Mutex::new(symbols.len()));
    let count_clone = Arc::clone(&count);

    let callbacks = Callbacks::default()
        .on_chart_data(move |(opt, data_points): (ChartOptions, Vec<DataPoint>)| {
            tracing::debug!("Received data batch with {} points", data_points.len());
            let sender = bar_tx_clone.clone();
            // Spawn the async work and return ()
            spawn(async move {
                // Acquire mutex lock to send data points
                let tx = sender.lock().await;
                if let Err(e) = tx.send((opt, data_points)).await {
                    tracing::error!("Failed to send data points: {}", e);
                }
            });
        })
        .on_symbol_info(move |symbol_info| {
            tracing::debug!("Received symbol info: {:?}", symbol_info);
            let sender = info_tx_clone.clone();
            // Spawn the async work and return ()
            spawn(async move {
                // Acquire mutex lock to send symbol info
                let tx = sender.lock().await;
                if let Err(e) = tx.send(symbol_info).await {
                    tracing::error!("Failed to send symbol info: {}", e);
                }
            });
        })
        .on_series_completed({
            // Create a clone for the closure
            let done_sender = Arc::clone(&done_tx);
            move |message: Vec<Value>| {
                let done_sender = Arc::clone(&done_sender);
                tracing::debug!("Series completed with message: {:?}", message);
                // Decrement the count of completed series
                let count_clone: Arc<Mutex<usize>> = Arc::clone(&count_clone);
                spawn(async move {
                    let mut count = count_clone.lock().await;
                    *count -= 1;
                    tracing::debug!("Remaining series count: {}", *count);
                    if *count > 0 {
                        return;
                    }
                    // Take the sender out of the Option and send the signal
                    if let Some(sender) = done_sender.lock().await.take() {
                        let _ = sender.send(());
                    }
                });
            }
        });

    // --------------------------------- WebSocket -------------------------------- //
    let client = WebSocketClient::default().set_callbacks(callbacks);
    let websocket = WebSocket::new()
        .server(server.unwrap_or(DataServer::ProData))
        .auth_token(auth_token)
        .client(client)
        .build()
        .await?;
    // Create an Arc to safely share the WebSocket between tasks
    let websocket = Arc::new(websocket);
    let websocket_for_loop = Arc::clone(&websocket);
    // Set the market before spawning the task
    for opt in opts.iter() {
        websocket.set_market(opt.clone()).await?;
    }
    // Spawn WebSocket subscription loop in background
    let subscription_handle = spawn(async move {
        websocket_for_loop.subscribe().await;
    });
    // --------------------------------- Accumulate -------------------------------- //
    let mut result: HashMap<String, ChartHistoricalData> = HashMap::new();
    // Create a future that completes when done_rx receives a value
    let mut completion_future = Box::pin(done_rx);
    // Use a loop with tokio::select! that doesn't consume the completion future
    loop {
        tokio::select! {
            Some((opt, data)) = bar_rx.recv() => {
                tracing::debug!("Processing batch of {} data points for symbol: {}", data.len(), opt.symbol);
                let id = format!("{}:{}", opt.exchange, opt.symbol);
                result.entry(id)
                    .or_insert_with(|| ChartHistoricalData::new(&opt))
                    .data
                    .extend(data);
            }
            _ = &mut completion_future => {
                tracing::debug!("Completion signal received");
                // Process any remaining data points
                while let Ok(Some((opt, data))) = timeout(
                   Duration::from_millis(100),
                    bar_rx.recv()
                ).await {
                    tracing::debug!("Processing final batch of {} data points for symbol: {}", data.len(), opt.symbol);
                    let id = format!("{}:{}", opt.exchange, opt.symbol);
                    result.entry(id)
                        .or_insert_with(|| ChartHistoricalData::new(&opt))
                        .data
                        .extend(data);
                }
                break;
            }
            Some(info) = info_rx.recv() => {
                tracing::debug!("Received symbol info: {:?}", info);
                result.entry(info.id.clone())
                    .or_insert_with(|| ChartHistoricalData::new(&based_opt))
                    .info = info.clone();
            }
            else => {
                tracing::debug!("No more data to receive");
                break;
            }
        }
    }
    // Ensure the WebSocket is properly closed
    if let Err(e) = websocket.delete().await {
        tracing::error!("Failed to close WebSocket: {}", e);
    }
    subscription_handle.abort();
    tracing::debug!(
        "Data collection completed with {} points",
        result.values().map(|d| d.data.len()).sum::<usize>()
    );
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Interval, MarketType, StocksType, list_symbols};
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
        // println!("Fetched data: {:?}", data.data.len());
        // println!("First data point: {:?}", data);
        assert!(!data.data.is_empty(), "Data should not be empty");
        assert_eq!(data.data.len(), 10, "Data length should be 10");
        assert_eq!(data.options.symbol, symbol, "Symbol should match");
        assert_eq!(data.options.interval, interval, "Interval should match");
        // println!("Symbol info: {:?}", data.info);
        Ok(())
    }

    #[tokio::test]
    async fn test_batch_fetch_chart_historical() -> anyhow::Result<()> {
        init();
        dotenv::dotenv().ok();
        let symbols = list_symbols(
            Some("HNX".to_string()),
            Some(MarketType::Stocks(StocksType::Common)),
            Some("VN".to_string()),
            None,
        )
        .await?;
        assert!(!symbols.is_empty(), "No symbols found");
        println!("Fetched {} symbols", symbols.len());
        let auth_token = std::env::var("TV_AUTH_TOKEN").expect("TV_AUTH_TOKEN is not set");
        let based_opt = ChartOptions::new_with("VCB", "HOSE", Interval::Daily).bar_count(10);
        let server = Some(DataServer::ProData);
        let data = batch_fetch_chart_historical(&auth_token, &symbols, based_opt, server).await?;
        // println!("Fetched data: {:?}", data);
        // assert_eq!(data.len(), 2, "Data length should be 2");
        // for (key, chart_data) in data.iter() {
        //     assert!(
        //         !chart_data.data.is_empty(),
        //         "Data for {} should not be empty",
        //         key
        //     );
        //     assert_eq!(
        //         chart_data.data.len(),
        //         10,
        //         "Data length for {} should be 10",
        //         key
        //     );
        //     assert_eq!(
        //         chart_data.options.symbol,
        //         key.split(':').last().unwrap(),
        //         "Symbol should match"
        //     );
        //     assert_eq!(
        //         chart_data.options.interval,
        //         Interval::Daily,
        //         "Interval should match"
        //     );
        // }
        println!("Fetched {:?} data points", data);
        Ok(())
    }
}
