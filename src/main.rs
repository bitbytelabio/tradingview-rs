use tradingview_rs::socket::SocketMessage;
use tradingview_rs::utils::{format_packet, parse_packet};
use tradingview_rs::UA;

use futures_util::{SinkExt, StreamExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_tungstenite::connect_async;
use tracing::{debug, error, info};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    // let mut quote_socket = tradingview_rs::quote::websocket::Socket::new(
    //     tradingview_rs::socket::DataServer::Data,
    //     None,
    // )
    // .await;

    // quote_socket.read_message().await;
}

fn optional_arg<Thing>(thing: Option<Thing>) -> Thing
where
    Thing: Default,
{
    thing.unwrap_or(Thing::default())
}
