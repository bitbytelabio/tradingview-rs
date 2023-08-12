use std::env;

use tradingview_rs::{
    chart::session::{ChartCallbackFn, WebSocket},
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

    socket.create_chart_session().await.unwrap();

    socket
        .resolve_symbol("sds_sym_1", "BINANCE:BTCUSDT", None, None, None)
        .await
        .unwrap();

    socket
        .create_series(
            "sds_1",
            "s2",
            "sds_sym_1",
            tradingview_rs::models::Interval::FourHours,
            20000,
        )
        .await
        .unwrap();

    socket.subscribe().await;
}
