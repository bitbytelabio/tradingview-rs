use dotenv::dotenv;
use std::env;

use tracing::info;
use tradingview::{
    chart,
    chart::ChartOptions,
    models::Interval,
    socket::{ DataServer, SocketSession },
    subscriber::Subscriber,
    quote,
};

#[tokio::main]
async fn main() {
    dotenv().ok();
    tracing_subscriber::fmt::init();
    let auth_token = env::var("TV_AUTH_TOKEN").unwrap();

    let socket = SocketSession::new(DataServer::ProData, auth_token).await.unwrap();

    let subscriber = Subscriber::new();

    let mut chart = chart::session::WebSocket::new(subscriber.clone(), socket.clone());

    let mut quote = quote::session::WebSocket::new(subscriber.clone(), socket.clone());

    // subscriber.subscribe(&mut chart, &mut socket);

    let opts = ChartOptions::new("BINANCE:BTCUSDT", Interval::OneMinute).bar_count(100);

    chart.set_market(opts).await.unwrap();

    quote.create_session().await.unwrap();
    quote.set_fields().await.unwrap();
    quote
        .add_symbols(
            vec![
                "SP:SPX",
                "BINANCE:BTCUSDT",
                "BINANCE:ETHUSDT",
                "BITSTAMP:ETHUSD",
                "NASDAQ:TSLA"
                // "BINANCE:B",
            ]
        ).await
        .unwrap();

    tokio::spawn(async move { chart.subscribe().await });
    tokio::spawn(async move { quote.subscribe().await });
    loop {
    }
}
