use dotenv::dotenv;
use std::env;
use tracing::info;
use tradingview::{
    chart::{ session::{ ChartCallbackFn, WebSocket }, ChartOptions, ChartSeriesData },
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

    let opts = ChartOptions::new("BINANCE:BTCUSDT", Interval::OneMinute)
        .replay_mode(true)
        .replay_from(1617235200);

    socket.set_market(opts).await.unwrap();

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
    let end = data.data.first().unwrap().timestamp;
    info!("on_chart_data: {:?} - {:?}", data.data.len(), end);
}

async fn on_symbol_resolved(data: tradingview::chart::SymbolInfo) {
    info!("on_symbol_resolved: {:?}", data);
}

async fn on_series_completed(data: tradingview::chart::SeriesCompletedMessage) {
    info!("on_series_completed: {:?}", data);
}
