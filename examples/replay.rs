use dotenv::dotenv;
use std::env;
use tracing::info;
use tradingview::{
    chart::{
        session::{ChartCallbackFn, WebSocket},
        ChartOptions, ChartSeries,
    },
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
        .connect(handlers)
        .await
        .unwrap();

    socket
        .set_market(
            "BINANCE:BTCUSDT",
            ChartOptions {
                resolution: Interval::OneMinute,
                bar_count: 50_000,
                replay_mode: Some(true),
                replay_from: Some(1688342400), //1689552000 // 1688342400 // 1687132800
                ..Default::default()
            },
        )
        .await
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

async fn on_chart_data(data: ChartSeries) -> Result<(), tradingview::error::Error> {
    let end = data.data.first().unwrap().timestamp;
    info!("on_chart_data: {:?} - {:?}", data.data.len(), end);
    Ok(())
}

async fn on_symbol_resolved(
    data: tradingview::chart::SymbolInfo,
) -> Result<(), tradingview::error::Error> {
    info!("on_symbol_resolved: {:?}", data);
    Ok(())
}

async fn on_series_completed(
    data: tradingview::chart::SeriesCompletedMessage,
) -> Result<(), tradingview::error::Error> {
    info!("on_series_completed: {:?}", data);
    Ok(())
}
