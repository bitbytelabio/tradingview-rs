use serde_json::Value;
use tracing::{debug, error, info, warn};
use tradingview_rs::quote::{websocket::QuoteSocket, QuoteSocketEvent};
use tradingview_rs::{prelude::*, socket::DataServer};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let mut binding = QuoteSocket::new(DataServer::Data, event_handler);
    let mut socket = binding
        // .quote_fields(vec!["lp".to_string()])
        .build()
        .await
        .unwrap();

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

    // use tradingview_rs::user::*;
    // // let user = User::new().build();
    // let user = User::new()
    //     // .credentials("", "")
    //     // .pro(true)
    //     .session(
    //         "2endn5wa0rw9gmu8bewxc6ymqbxhrbw2",
    //         "v1:+MY3Ea/emfmC56ZMMu2Q5PO+kW3ZB36QxiW2U6nEhIw=",
    //     )
    //     .build()
    //     .await;
    // debug!("user: {:#?}", user);
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
