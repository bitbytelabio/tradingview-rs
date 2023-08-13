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
                resolution: Interval::FourHours,
                bar_count: 5,
                replay_mode: Some(true),
                replay_from: Some(1640224799),
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

async fn on_chart_data(data: ChartSeries) -> Result<(), Box<dyn std::error::Error>> {
    info!("on_chart_data: {:?}", data);
    Ok(())
}
