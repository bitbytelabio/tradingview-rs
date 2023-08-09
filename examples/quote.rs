use std::env;

use tracing::info;
use tradingview_rs::quote::{session::WebSocket, QuotePayload, QuoteSocketEvent};
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

    let mut socket = WebSocket::new()
        .server(DataServer::ProData)
        .auth_token(user.auth_token)
        .connect(event_handler)
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

fn event_handler(event: QuoteSocketEvent, data: QuotePayload) -> Result<()> {
    match event {
        QuoteSocketEvent::Data => {
            // debug!("data: {:#?}", &data);
            // let data2 = data.get("v").unwrap();
            // let quote: QuoteValue = serde_json::from_value(data2.clone()).unwrap();
            // info!("quote: {:#?}", quote);
            info!("Data: {:#?}", data);
        }
        QuoteSocketEvent::Loaded => {
            info!("loaded: {:#?}", data);
        }
        QuoteSocketEvent::Error(_) => todo!(),
    }
    Ok(())
}
