use dotenv::dotenv;
use std::env;

use tradingview::{
    callback::Callbacks,
    chart::ChartOptions,
    pine_indicator::ScriptType,
    socket::DataServer,
    websocket::{WebSocket, WebSocketClient},
    Interval, QuoteValue,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    tracing_subscriber::fmt::init();
    let auth_token = env::var("TV_AUTH_TOKEN").expect("TV_AUTH_TOKEN is not set");

    let quote_callback = |data: QuoteValue| async move {
        println!("{:#?}", data);
    };

    let callbacks = Callbacks::default().on_quote_data(quote_callback);

    let client = WebSocketClient::default().set_callbacks(callbacks);

    let mut websocket = WebSocket::new()
        .server(DataServer::ProData)
        .auth_token(&auth_token)
        .client(client)
        .build()
        .await
        .unwrap();

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

    websocket
        .set_market(opts)
        .await?
        .set_market(opts2)
        .await?
        .set_market(opts3)
        .await?;

    websocket
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

    tokio::spawn(async move { websocket.subscribe().await });

    loop {}
}
