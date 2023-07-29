use serde_json::Value;
use tracing::info;
use tradingview_rs::quote::{websocket::QuoteSocket, QuoteSocketEvent};
use tradingview_rs::{prelude::*, socket::DataServer};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let mut binding = QuoteSocket::new(DataServer::Data, event_handler);
    let mut socket = binding.build().await.unwrap();

    socket
        .quote_add_symbols(vec![
            "BINANCE:BTCUSDT".to_string(),
            "BINANCE:ETHUSDT".to_string(),
            "BITSTAMP:ETHUSD".to_string(),
            "NASDAQ:TSLA".to_string(),
        ])
        .await
        .unwrap();

    socket.event_loop().await;
}

fn event_handler(event: QuoteSocketEvent, data: Value) -> Result<()> {
    match event {
        QuoteSocketEvent::Data => {
            info!("data: {:#?}", data);
        }
        QuoteSocketEvent::Loaded => {
            info!("loaded: {:#?}", data);
        }
        QuoteSocketEvent::Error => todo!(),
    }
    Ok(())
}
