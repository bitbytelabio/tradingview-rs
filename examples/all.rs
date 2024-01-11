use dotenv::dotenv;
use std::env;

use tradingview::{
    chart,
    chart::ChartOptions,
    data_loader::DataLoader,
    models::{pine_indicator::ScriptType, Interval},
    quote,
    socket::{DataServer, SocketSession},
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    tracing_subscriber::fmt::init();
    let auth_token = env::var("TV_AUTH_TOKEN").unwrap();

    let socket = SocketSession::new(DataServer::ProData, auth_token).await?;

    let publisher: DataLoader = DataLoader::default();

    let mut chart = chart::session::WebSocket::new(publisher.clone(), socket.clone());
    // let mut quote = quote::session::WebSocket::new(publisher.clone(), socket.clone());

    // subscriber.subscribe(&mut chart, &mut socket);

    // let opts = ChartOptions::new("BINANCE:BTCUSDT", Interval::OneMinute).bar_count(100);
    // let opts2 = ChartOptions::new("BINANCE:BTCUSDT", Interval::Daily).study_config(
    //     "STD;Candlestick%1Pattern%1Bearish%1Abandoned%1Baby",
    //     "33.0",
    //     ScriptType::IntervalScript,
    // );
    let opts3 = ChartOptions::new("BINANCE:BTCUSDT", Interval::OneHour).bar_count(1);
    // .replay_mode(true)
    // .replay_from(1698624060);

    chart
        // .set_market(opts)
        // .await?
        // .set_market(opts2)
        // .await?
        .set_market(opts3)
        .await?;

    // quote
    // .create_session()
    // .await?
    // .set_fields()
    // .await?
    // .add_symbols(vec![
    //     "SP:SPX",
    //     "BINANCE:BTCUSDT",
    //     "BINANCE:ETHUSDT",
    //     "BITSTAMP:ETHUSD",
    //     "NASDAQ:TSLA", // "BINANCE:B",
    // ])
    // .await?;

    tokio::spawn(async move { chart.clone().subscribe().await });
    // tokio::spawn(async move { quote.clone().subscribe().await });

    loop {}
}
