use rust_socketio::{ClientBuilder, Payload, RawClient};
use serde_json::json;
use tracing::{error, info};
use tracing_subscriber::fmt::init as tracing_init;

fn main() {
    tracing_init();

    let callback = |payload: Payload, _: RawClient| {
        info!("Received payload: {:#?}", &payload);
        match payload {
            Payload::String(str) => println!("Received: {}", str),
            Payload::Binary(bin_data) => println!("Received bytes: {:#?}", bin_data),
        };
    };

    let socket = ClientBuilder::new("https://data.tradingview.com/")
        .namespace("/websocket")
        .opening_header("origin", "https://data.tradingview.com")
        .on("connect", callback)
        .on("error", |err, _| eprintln!("Error: {:#?}", err))
        .connect();

    match socket {
        Ok(raw_client) => info!("Connected to the server"),
        Err(err) => error!("Connection error: {:#?}", err),
    }
}
