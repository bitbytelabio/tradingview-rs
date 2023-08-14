use std::env;

use tracing::info;
use tradingview_rs::{
    chart::{
        session::{ChartCallbackFn, Options, WebSocket},
        ChartSeries,
    },
    models::Interval,
    socket::DataServer,
    user::User,
};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let session = env::var("TV_SESSION").unwrap();
    let signature = env::var("TV_SIGNATURE").unwrap();

    let user = User::build()
        .session(&session, &signature)
        .get()
        .await
        .unwrap();

    let handlers = ChartCallbackFn {
        on_chart_data: Box::new(|data| Box::pin(on_chart_data(data))),
        on_symbol_resolved: Box::new(|data| Box::pin(on_symbol_resolved(data))),
        on_series_completed: Box::new(|data| Box::pin(on_series_completed(data))),
    };

    let mut socket = WebSocket::build()
        .server(DataServer::ProData)
        .auth_token(user.auth_token)
        .connect(handlers)
        .await
        .unwrap();

    socket
        .set_market(
            "BINANCE:BTCUSDT",
            Options {
                resolution: Interval::OneMinute,
                bar_count: 50_000,
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
    //     .set_series(tradingview_rs::models::Interval::FourHours, 20000, None)
    //     .await
    //     .unwrap();

    socket.subscribe().await;
}

async fn on_chart_data(data: ChartSeries) -> Result<(), tradingview_rs::error::Error> {
    // info!("on_chart_data: {:?}", data);
    let end = data.data.first().unwrap().0;
    info!("on_chart_data: {:?} - {:?}", data.data.len(), end);
    Ok(())
}

async fn on_symbol_resolved(
    data: tradingview_rs::chart::SymbolInfo,
) -> Result<(), tradingview_rs::error::Error> {
    info!("on_symbol_resolved: {:?}", data);
    Ok(())
}

async fn on_series_completed(
    data: tradingview_rs::chart::SeriesCompletedMessage,
) -> Result<(), tradingview_rs::error::Error> {
    info!("on_series_completed: {:?}", data);
    Ok(())
}
