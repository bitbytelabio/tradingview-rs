use std::env;

use tradingview_rs::{
    chart::session::{ChartCallbackFn, Options, WebSocket},
    models::Interval,
    socket::DataServer,
    socket::SocketSession,
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

    let handlers = ChartCallbackFn {};

    let session = SocketSession::new(DataServer::ProData, user.auth_token)
        .await
        .unwrap();

    let mut socket = WebSocket::build()
        .socket(session)
        .connect(handlers)
        .await
        .unwrap();

    socket
        .set_market(
            "BINANCE:ETHUSDT",
            Options {
                resolution: Interval::FourHours,
                bar_count: 5,
                range: Some("60M".to_string()),
                ..Default::default()
            },
        )
        .await
        .unwrap();

    socket.subscribe().await;
}
