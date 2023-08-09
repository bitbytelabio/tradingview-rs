use std::env;

use tracing::info;
use tradingview_rs::error::TradingViewError;
use tradingview_rs::quote::{
    session::{QuoteCallbackFn, WebSocket},
    QuotePayload, QuoteSocketMessage,
};
use tradingview_rs::socket::DataServer;
use tradingview_rs::user::User;
type Result<T> = std::result::Result<T, tradingview_rs::error::Error>;

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

    let handlers = QuoteCallbackFn {
        data: Box::new(on_data),
        loaded: Box::new(on_loaded),
        error: Box::new(on_error),
    };

    let mut socket = WebSocket::new()
        .server(DataServer::ProData)
        .auth_token(user.auth_token)
        .connect(handlers)
        .await
        .unwrap();

    socket.create_session().await.unwrap();
    socket.set_fields().await.unwrap();

    socket
        .add_symbols(vec![
            "BINANCE:BTCUSDT",
            "BINANCE:ETHUSDT",
            "BITSTAMP:ETHUSD",
            "NASDAQ:TSLA",
        ])
        .await
        .unwrap();

    socket.subscribe().await;
}

fn on_data(data: QuotePayload) -> Result<()> {
    info!("Data: {:#?}", data);
    Ok(())
}

fn on_loaded(msg: QuoteSocketMessage) -> Result<()> {
    info!("Data: {:#?}", msg);
    Ok(())
}

fn on_error(err: TradingViewError) -> Result<()> {
    info!("Error: {:#?}", err);
    Ok(())
}
