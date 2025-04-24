use crate::{
    DataPoint, Interval, Result,
    callback::Callbacks,
    chart::ChartOptions,
    socket::{DataServer, TradingViewDataEvent},
    websocket::{WebSocket, WebSocketClient},
};
pub use models::*;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc, oneshot};

mod models;

/// Fetch historical chart data from TradingView
pub async fn fetch_chart_historical(
    auth_token: &str,
    symbol: &str,
    exchange: &str,
    interval: Interval,
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
        .auth_token(&auth_token)
        .client(client)
        .build()
        .await?;

    // Create an Arc to safely share the WebSocket between tasks
    let websocket = Arc::new(Mutex::new(websocket));
    let websocket_for_loop = Arc::clone(&websocket);
    // Configure chart options
    let chart_opts = ChartOptions::new(symbol, interval).bar_count(500_000);

    // Set the market before spawning the task
    {
        let mut ws = websocket.lock().await;
        ws.set_market(chart_opts).await?;
    }

    // Spawn WebSocket subscription loop in background
    let subscription_handle = tokio::spawn(async move {
        let mut ws = websocket_for_loop.lock().await;
        ws.subscribe().await;
    });

    // --------------------------------- Accumulate -------------------------------- //
    let mut result = ChartHistoricalData::new(symbol, exchange, interval);
    // Collect data until we receive the completion signal
    tokio::select! {
        _ = async {
            while let Some(points) = bar_rx.recv().await {
                result.add_points(points);
            }
        } => {}
        _ = done_rx => {
            // Series completed signal received
        }
    }

    // Abort the subscription task and close the websocket
    subscription_handle.abort();
    let mut ws = websocket.lock().await;
    let _ = ws.close().await.ok();

    Ok(result)
}

#[cfg(test)]
mod tests {
    use anyhow::Ok;

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
        let server = Some(DataServer::ProData);
        let data = fetch_chart_historical(&auth_token, symbol, exchange, interval, server).await?;
        println!("Fetched data: {:?}", data);

        Ok(())
    }
}
