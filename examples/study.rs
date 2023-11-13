use dotenv::dotenv;
use std::env;
use tracing::info;
use tradingview::{
    chart::{ session::{ ChartCallbackFn, WebSocket }, ChartOptions, ChartSeriesData },
    models::{ Interval, pine_indicator::ScriptType },
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

    let opts = ChartOptions::new("BINANCE:BTCUSDT", Interval::Daily).study_config(
        "STD;Candlestick%1Pattern%1Bearish%1Abandoned%1Baby",
        "33.0",
        ScriptType::IntervalScript
    );

    socket.set_market(opts).await.unwrap();

    socket.subscribe().await;
}

async fn on_chart_data(_data: ChartSeriesData) {
    // info!("on_chart_data: {:?}", data);
    // let end = data.data.first().unwrap().0;
    // info!("on_chart_data: {:?} - {:?}", data.data.len(), end);
}

async fn on_symbol_resolved(data: tradingview::chart::SymbolInfo) {
    info!("on_symbol_resolved: {:?}", data);
}

async fn on_series_completed(data: tradingview::chart::SeriesCompletedMessage) {
    info!("on_series_completed: {:?}", data);
}
