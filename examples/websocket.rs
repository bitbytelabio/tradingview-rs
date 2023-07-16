use datafeed::client::SocketMessage;
use datafeed::utils::protocol::{format_packet, parse_packet};
// use futures_util::SinkExt;
use serde_json::Value;
use tracing::debug;
use tungstenite::{client::IntoClientRequest, connect, stream::NoDelay, Message};

#[tokio::main]
async fn main() {
    // Initialize tracing subscriber
    tracing_subscriber::fmt::init();

    datafeed::quote::models::QuoteSession::load();
}
