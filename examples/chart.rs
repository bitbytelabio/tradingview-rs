use std::env;
use tradingview_rs::chart::{websocket::ChartSocket, ChartEvent};
use tradingview_rs::socket::{self, DataServer};
use tradingview_rs::user::User;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let session = env::var("TV_SESSION").unwrap();
    let signature = env::var("TV_SIGNATURE").unwrap();

    let user = User::new()
        .session(&session, &signature)
        .build()
        .await
        .unwrap();

    let mut socket = ChartSocket::new(DataServer::Data)
        .auth_token(user.auth_token)
        .build()
        .await
        .unwrap();

    socket.event_loop().await;
}
