use dotenv::dotenv;
use std::{
    env,
    sync::{Arc, Mutex},
};

use tradingview::{
    Interval, OHLCV,
    callback::EventCallback,
    chart::ChartOptions,
    fetch_chart_data,
    socket::DataServer,
    websocket::{WebSocket, WebSocketClient},
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let auth_token = env::var("TV_AUTH_TOKEN").expect("TV_AUTH_TOKEN is not set");

    let option = ChartOptions::new_with("BTCUSDT", "BINANCE", Interval::OneHour);

    let mut data = fetch_chart_data()
        .auth_token(&auth_token)
        .options(option)
        .server(DataServer::ProData)
        .call()
        .await?
        .data;

    // Sort data points by timestamp (ascending order - earliest first)
    data.sort_by_key(|a| a.timestamp());

    // Get the earliest timestamp for replay_from
    let earliest_timestamp = data.first().map(|point| point.timestamp()).unwrap_or(0);

    println!("Earliest timestamp: {}", earliest_timestamp);
    println!("Total data points: {}", data.len());
    let data = Arc::new(Mutex::new(data));

    // Now create a new option with replay mode using the earliest timestamp
    let replay_option = ChartOptions::new_with("BTCUSDT", "BINANCE", Interval::OneHour)
        .replay_mode(true)
        .replay_from(earliest_timestamp);

    let callbacks: EventCallback = EventCallback::default()
        .on_error(|(err, mess)| {
            tracing::error!("Error occurred: {} - {:?}", err, mess);
        })
        .on_replay_ok(|data| {
            tracing::info!("Replay started successfully: {:?}", data);
        })
        .on_replay_point(|data| {
            tracing::info!("Replay point received: {:?}", data);
        })
        .on_chart_data({
            let data = Arc::clone(&data);
            move |(series_info, data_points)| {
                tracing::debug!(
                    "Chart data received for series: {:?}, Data points count: {}",
                    series_info,
                    data_points.len()
                );
                let mut data = data.lock().unwrap();
                data.extend(data_points);

                // sort the data points by timestamp
                data.sort_by_key(|a| a.timestamp());
                // Remove duplicates based on timestamp
                data.dedup_by_key(|point| point.timestamp());
                tracing::debug!("Total data points after update: {}", data.len());
            }
        })
        .on_series_loading(|series| {
            tracing::info!("Series loading: {:?}", series);
        })
        .on_series_completed(|series| {
            tracing::info!("Series completed: {:?}", series);
        });
    let client = WebSocketClient::default().set_callbacks(callbacks);

    let websocket = WebSocket::new()
        .server(DataServer::ProData)
        .auth_token(&auth_token)
        .client(client)
        .build()
        .await?;

    websocket.set_market(replay_option).await?;

    websocket.subscribe().await;

    Ok(())
}
