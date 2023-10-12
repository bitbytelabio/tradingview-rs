use dotenv::dotenv;
use std::env;
use tracing::info;
use tradingview::{
    chart::{ session::{ ChartCallbackFn, WebSocket }, ChartOptions, ChartSeriesData },
    models::Interval,
    socket::DataServer,
};
use polars::prelude::*;

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
        .set_market(ChartOptions {
            symbol: "BINANCE:BTCUSDT".to_string(),
            interval: Interval::OneMinute,
            bar_count: 50_000,

            fetch_all_data: true,
            fetch_data_count: 100_000,

            ..Default::default()
        }).await
        .unwrap();

    // socket
    //     .set_market(
    //         "BINANCE:ETHUSDT",
    //         Options {
    //             resolution: Interval::FourHours,
    //             bar_count: 5,
    //             range: Some("60M".to_string()),
    //             ..Default::default()
    //         },
    //     )
    //     .await
    //     .unwrap();

    // socket
    //     .resolve_symbol("ser_1", "BINANCE:BTCUSDT", None, None, None)
    //     .await
    //     .unwrap();
    // socket
    //     .set_series(tradingview::models::Interval::FourHours, 20000, None)
    //     .await
    //     .unwrap();

    socket.subscribe().await;
}

async fn on_chart_data(data: ChartSeriesData) {
    let timestamp: Vec<i64> = data.data
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
            Series::new("timestamp", timestamp),
            Series::new("open", opens),
            Series::new("high", highs),
            Series::new("low", lows),
            Series::new("close", closes),
            Series::new("volume", volumes)
        ]
    ).unwrap();

    df.with_column(
        df
            .column("timestamp")
            .unwrap()
            .cast(&DataType::Datetime(TimeUnit::Milliseconds, Some("Utc".to_owned())))
            .unwrap()
    ).unwrap();
    df.rename("timestamp", "date").unwrap();

    let csv_out = "tmp/BINANCE_BTCUSDT.csv";
    if std::fs::metadata(csv_out).is_err() {
        let f = std::fs::File::create(csv_out).unwrap();
        CsvWriter::new(f).finish(&mut df).unwrap();
    }
}

async fn on_symbol_resolved(data: tradingview::chart::SymbolInfo) {
    info!("on_symbol_resolved: {:?}", data);
}

async fn on_series_completed(data: tradingview::chart::SeriesCompletedMessage) {
    info!("on_series_completed: {:?}", data);
}
