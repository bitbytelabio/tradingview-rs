use dotenv::dotenv;
use std::{ env, process::exit };

use polars::prelude::*;

use tracing::info;
use tradingview::{
    chart::{ session::{ ChartCallbackFn, WebSocket }, ChartOptions, ChartSeries },
    models::Interval,
    socket::DataServer,
};

#[tokio::main]
async fn main() {
    dotenv().ok();
    tracing_subscriber::fmt::init();
    let auth_token = env::var("TV_AUTH_TOKEN").unwrap();

    let handlers = ChartCallbackFn {
        on_chart_data: Box::new(|data| Box::pin(on_chart_data(data))),
        on_symbol_resolved: Box::new(|data| Box::pin(on_symbol_resolved(data))),
        on_series_completed: Box::new(|data| Box::pin(on_series_completed(data))),
    };

    let mut socket = WebSocket::build()
        .server(DataServer::ProData)
        .auth_token(auth_token)
        .connect(handlers).await
        .unwrap();

    socket
        .set_market("NASDAQ:AMZN", ChartOptions {
            resolution: Interval::Daily,
            bar_count: 50_000,
            ..Default::default()
        }).await
        .unwrap();
    socket.subscribe().await;
}

async fn on_chart_data(data: ChartSeries) -> Result<(), tradingview::error::Error> {
    // Assuming you have the OHLCV data in a Vec<OHLCV> named `data`

    // Extract the individual fields from OHLCV objects
    let timestamps: Vec<i64> = data.data
        .iter()
        .map(|item| item.timestamp)
        .collect();
    let opens: Vec<f64> = data.data
        .iter()
        .map(|item| item.open)
        .collect();
    let highs: Vec<f64> = data.data
        .iter()
        .map(|item| item.high)
        .collect();
    let lows: Vec<f64> = data.data
        .iter()
        .map(|item| item.low)
        .collect();
    let closes: Vec<f64> = data.data
        .iter()
        .map(|item| item.close)
        .collect();
    let volumes: Vec<f64> = data.data
        .iter()
        .map(|item| item.volume)
        .collect();

    // Create a Polars DataFrame
    let mut df = DataFrame::new(
        vec![
            Series::new("timestamp", timestamps),
            Series::new("open", opens),
            Series::new("high", highs),
            Series::new("low", lows),
            Series::new("close", closes),
            Series::new("volume", volumes)
        ]
    ).unwrap();

    let csv_out = "tmp/nasdaq_amzn.csv";
    if std::fs::metadata(csv_out).is_err() {
        let f = std::fs::File::create(csv_out).unwrap();
        CsvWriter::new(f).finish(&mut df).unwrap();
    }

    Ok(())
}

async fn on_symbol_resolved(
    data: tradingview::chart::SymbolInfo
) -> Result<(), tradingview::error::Error> {
    info!("on_symbol_resolved: {:?}", data);
    Ok(())
}

async fn on_series_completed(
    data: tradingview::chart::SeriesCompletedMessage
) -> Result<(), tradingview::error::Error> {
    info!("on_series_completed: {:?}", data);
    exit(0);
}
