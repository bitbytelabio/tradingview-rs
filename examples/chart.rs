use std::env;

use tradingview_rs::{
    chart::session::{ChartCallbackFn, WebSocket},
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

    let handlers = ChartCallbackFn {};

    let mut socket = WebSocket::build()
        .server(DataServer::ProData)
        .auth_token(user.auth_token)
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
            tradingview_rs::Interval::FourHours,
            20000,
        )
        .await
        .unwrap();

    socket.subscribe().await;
}
