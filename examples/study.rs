use dotenv::dotenv;
use std::env;
use tracing::info;
use tradingview::{
    chart::{ session::{ ChartCallbackFn, WebSocket }, ChartOptions, ChartSeriesData, StudyOptions },
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
        .set_market(ChartOptions {
            symbol: "BINANCE:BTCUSDT".to_string(),
            interval: Interval::Daily,
            bar_count: 1,
            study_config: Some(StudyOptions {
                script_id: "STD;Candlestick%1Pattern%1Bearish%1Abandoned%1Baby".to_string(),
                script_version: "33.0".to_string(),
                script_type: tradingview::models::pine_indicator::ScriptType::IntervalScript,
            }),
            ..Default::default()
        }).await
        .unwrap();

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
