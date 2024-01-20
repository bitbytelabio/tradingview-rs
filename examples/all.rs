use dotenv::dotenv;
use std::env;

use tradingview::{
    chart::ChartOptions,
    pine_indicator::ScriptType,
    socket::{DataServer, SocketSession},
    websocket::{WSClient, WebSocket},
    Interval,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    tracing_subscriber::fmt::init();
    let auth_token = env::var("TV_AUTH_TOKEN").unwrap();

    let socket = SocketSession::new(DataServer::ProData, auth_token).await?;

    let publisher: WSClient = WSClient::default();

    let mut connector = WebSocket::new_with_session(publisher.clone(), socket.clone());

    let opts = ChartOptions::new("BINANCE:BTCUSDT", Interval::OneMinute).bar_count(100);
    let opts2 = ChartOptions::new("BINANCE:BTCUSDT", Interval::Daily)
        .bar_count(1)
        .study_config(
            "STD;Candlestick%1Pattern%1Bearish%1Abandoned%1Baby",
            "33.0",
            ScriptType::IntervalScript,
        );
    let opts3 = ChartOptions::new("BINANCE:BTCUSDT", Interval::OneHour)
        .replay_mode(true)
        .replay_from(1698624060);

    connector
        .set_market(opts)
        .await?
        .set_market(opts2)
        .await?
        .set_market(opts3)
        .await?;

    connector
        .create_quote_session()
        .await?
        .set_fields()
        .await?
        .add_symbols(vec![
            "SP:SPX",
            "BINANCE:BTCUSDT",
            "BINANCE:ETHUSDT",
            "BITSTAMP:ETHUSD",
            "NASDAQ:TSLA", // "BINANCE:B",
        ])
        .await?;

    tokio::spawn(async move { connector.clone().subscribe().await });

    loop {}
}
