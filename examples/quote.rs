use serde_json::Value;
use tracing::info;
use tradingview_rs::quote::{websocket::QuoteSocket, QuoteEvent};
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

fn event_handler(event: QuoteEvent, data: Value) -> Result<()> {
    match event {
        QuoteEvent::Data => {
            info!("data: {:#?}", data);
        }
        QuoteEvent::Loaded => {
            info!("loaded: {:#?}", data);
        }
        QuoteEvent::Error => todo!(),
    }
    Ok(())
}
