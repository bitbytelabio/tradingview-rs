use crate::{
    ChartHistoricalData, DataPoint, Result,
    callback::Callbacks,
    chart::ChartOptions,
    socket::{DataServer, TradingViewDataEvent},
    websocket::{WebSocket, WebSocketClient},
};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc, oneshot};

/// Fetch historical chart data from TradingView
pub async fn fetch_chart_historical(
    auth_token: &str,
    options: ChartOptions,
    server: Option<DataServer>,
) -> Result<ChartHistoricalData> {
    // Use a bounded channel with sufficient capacity to provide backpressure if needed
    let (bar_tx, mut bar_rx) = mpsc::channel::<Vec<DataPoint>>(100);

    // One-shot channel for series completion signal
    let (done_tx, done_rx) = oneshot::channel::<()>();

    // Create callback functions using Arc<Mutex> to ensure thread-safe access
    let bar_tx = Arc::new(Mutex::new(bar_tx));
    let done_tx = Arc::new(Mutex::new(Some(done_tx)));
    // Clone for callbacks to move into closures
    let bar_tx_clone = Arc::clone(&bar_tx);

    let callbacks = Callbacks::default()
        .on_chart_data(move |(_, data_points): (ChartOptions, Vec<DataPoint>)| {
            tracing::debug!("Received data batch with {} points", data_points.len());
            let sender = bar_tx_clone.clone();
            let points = data_points.to_vec();

            // Spawn the async work and return ()
            tokio::spawn(async move {
                // Acquire mutex lock to send data points
                let tx = sender.lock().await;
                if let Err(e) = tx.send(points).await {
                    tracing::error!("Failed to send data points: {}", e);
                }
            });
        })
        .on_other_event({
            // Create a clone for the closure
            let done_sender = Arc::clone(&done_tx);

            move |(event, _): (TradingViewDataEvent, Vec<Value>)| {
                let done_sender = Arc::clone(&done_sender);

                tokio::spawn(async move {
                    if matches!(event, TradingViewDataEvent::OnSeriesCompleted) {
                        // Wait a small amount of time to ensure all data is processed
                        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

                        // Take the sender out of the Option and send the signal
                        if let Some(sender) = done_sender.lock().await.take() {
                            let _ = sender.send(());
                        }
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
    let subscription_handle = tokio::spawn(async move {
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
                while let Ok(Some(points)) = tokio::time::timeout(
                    tokio::time::Duration::from_millis(100),
                    bar_rx.recv()
                ).await {
                    tracing::debug!("Processing final batch of {} data points", points.len());
                    result.data.extend(points);
                }
                break;
            }
        }
    }

    // Ensure the WebSocket is properly closed
    let _ = websocket.delete().await;
    subscription_handle.abort();

    // Give a little extra time for any final cleanup
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    tracing::debug!(
        "Data collection completed with {} points",
        result.data.len()
    );
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Interval;
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
        let option = ChartOptions::new(symbol, exchange, interval).bar_count(100);
        let server = Some(DataServer::ProData);
        let data = fetch_chart_historical(&auth_token, option, server).await?;
        // println!("Fetched data: {:?}", data.data.len());
        // println!("First data point: {:?}", data);
        assert!(!data.data.is_empty(), "Data should not be empty");
        assert_eq!(data.data.len(), 100, "Data length should be 10");
        assert_eq!(data.options.symbol, symbol, "Symbol should match");
        assert_eq!(data.options.interval, interval, "Interval should match");
        Ok(())
    }
}
