use tradingview_rs::chart::{websocket::ChartSocket, ChartEvent};
use tradingview_rs::socket::DataServer;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let mut binding = ChartSocket::new(DataServer::Data);
    let mut socket = binding.build().await.unwrap();

    socket.event_loop().await;
}
