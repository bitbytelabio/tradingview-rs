use crate::{
    ChartHistoricalData, DataPoint, Result,
    callback::Callbacks,
    chart::ChartOptions,
    socket::{DataServer, TradingViewDataEvent},
    websocket::{WebSocket, WebSocketClient},
};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock, mpsc, oneshot};

/// Fetch historical chart data from TradingView
pub async fn fetch_chart_historical(
    auth_token: &str,
    options: ChartOptions,
    server: Option<DataServer>,
) -> Result<ChartHistoricalData> {
    // Channel to receive incremental bar batches
    let (bar_tx, mut bar_rx) = mpsc::unbounded_channel::<Vec<DataPoint>>();

    // One-shot channel for series completion signal
    let (done_tx, done_rx) = oneshot::channel::<()>();

    // Create callback functions using Arc to ensure they live long enough
    let bar_tx = Arc::new(bar_tx);
    // Wrap the oneshot sender in an Arc<Mutex> since it's not Clone
    let done_tx = Arc::new(Mutex::new(Some(done_tx)));

    // Clone for callbacks to move into closures
    let bar_tx_clone = Arc::clone(&bar_tx);

    let callbacks = Callbacks::default()
        .on_chart_data(move |(_, data_points): (ChartOptions, Vec<DataPoint>)| {
            let sender = bar_tx_clone.clone();
            async move {
                let _ = sender.send(data_points);
            }
        })
        .on_other_event({
            // Create a clone for the closure
            let done_sender = Arc::clone(&done_tx);

            move |(event, _): (TradingViewDataEvent, Vec<Value>)| {
                let done_sender = Arc::clone(&done_sender);
                async move {
                    if matches!(event, TradingViewDataEvent::OnSeriesCompleted) {
                        // Take the sender out of the Option and send the signal
                        if let Some(sender) = done_sender.lock().await.take() {
                            let _ = sender.send(());
                        }
                    }
                }
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
    let websocket = Arc::new(RwLock::new(websocket));
    let websocket_for_loop = Arc::clone(&websocket);

    // Set the market before spawning the task
    {
        let ws = websocket.read().await;
        ws.set_market(options.clone()).await?;
    }

    // Spawn WebSocket subscription loop in background
    let subscription_handle = tokio::spawn(async move {
        let ws = websocket_for_loop.read().await;
        ws.subscribe().await;
    });

    // --------------------------------- Accumulate -------------------------------- //
    let mut result = ChartHistoricalData::new(&options);
    // Collect data until we receive the completion signal
    tokio::select! {
        _ = async {
            while let Some(points) = bar_rx.recv().await {
                tracing::debug!("Received data points: {:?}", points);
                result.data.extend(points);
            }
        } => {
            // Data collection completed
            tracing::debug!("Data collection completed");
        }
        _ = done_rx => {
            // Completion signal received
            tracing::debug!("Received completion signal");

            let ws = websocket.read().await;
            let _ = ws.delete().await.ok();
        }
    }

    subscription_handle.abort();
    Ok(result)
}

#[cfg(test)]
mod tests {
    use anyhow::Ok;

    use crate::Interval;

    use super::*;

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
        let symbol = "FPT";
        let exchange = "HOSE";
        let interval = Interval::Daily;
        let option = ChartOptions::new(symbol, exchange, interval).bar_count(10);
        let server = Some(DataServer::ProData);
        let data = fetch_chart_historical(&auth_token, option, server).await?;
        // println!("Fetched data: {:?}", data.data.len());
        // println!("First data point: {:?}", data);
        assert!(!data.data.is_empty(), "Data should not be empty");
        assert_eq!(data.data.len(), 10, "Data length should be 10");
        assert_eq!(data.options.symbol, symbol, "Symbol should match");
        assert_eq!(data.options.interval, interval, "Interval should match");
        Ok(())
    }
}
