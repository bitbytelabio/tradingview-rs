use dotenv::dotenv;
use std::env;

use tracing::info;
use tradingview::{
    chart,
    chart::ChartOptions,
    models::{ Interval, pine_indicator::ScriptType },
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
    let opts2 = ChartOptions::new("BINANCE:BTCUSDT", Interval::Daily).study_config(
        "STD;Candlestick%1Pattern%1Bearish%1Abandoned%1Baby",
        "33.0",
        ScriptType::IntervalScript
    );
    let opts3 = ChartOptions::new("BINANCE:BTCUSDT", Interval::OneHour)
        .replay_mode(true)
        .replay_from(1698624060);
    chart
        .set_market(opts).await
        .unwrap()
        .set_market(opts2).await
        .unwrap()
        .set_market(opts3).await
        .unwrap();
    quote
        .create_session().await
        .unwrap()
        .set_fields().await
        .unwrap()
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
    tokio::spawn(async move { chart.clone().subscribe().await });
    tokio::spawn(async move { quote.clone().subscribe().await });

    loop {
    }
}
