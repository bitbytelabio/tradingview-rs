use std::env;

use tradingview_rs::{
    chart::session::{ChartCallbackFn, WebSocket},
    socket::{DataServer, Socket},
    user::User,
    SessionType,
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
        .resolve_symbol(
            "sds_sym_1",
            "HOSE:FPT",
            Some(tradingview_rs::MarketAdjustment::Splits),
            Some(iso_currency::Currency::VND),
            Some(SessionType::Regular),
        )
        .await
        .unwrap();

    socket
        .create_series(
            "sds_1",
            "s1",
            "sds_sym_1",
            tradingview_rs::Interval::FourHours,
            100,
        )
        .await
        .unwrap();

    socket.event_loop().await;
}
