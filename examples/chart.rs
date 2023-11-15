use dotenv::dotenv;
use std::env;

use tracing::info;
use tradingview::{
    chart,
    chart::ChartOptions,
    models::Interval,
    socket::{ DataServer, SocketSession },
    subscriber::Subscriber,
};

#[tokio::main]
async fn main() {
    dotenv().ok();
    tracing_subscriber::fmt::init();
    let auth_token = env::var("TV_AUTH_TOKEN").unwrap();

    let socket = SocketSession::new(DataServer::ProData, auth_token).await.unwrap();

    let subscriber = Subscriber::default();

    let mut chart = chart::session::WebSocket::new(subscriber, socket);

    // subscriber.subscribe(&mut chart, &mut socket);

    let opts = ChartOptions::new("BINANCE:BTCUSDT", Interval::OneMinute).bar_count(100);

    chart.set_market(opts).await.unwrap();

    chart.subscribe().await;
}
