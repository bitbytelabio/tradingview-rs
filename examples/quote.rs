use dotenv::dotenv;
use std::collections::HashMap;
use std::env;
use tracing::{error, info};
use tradingview_rs::quote::session::{QuoteCallbackFn, WebSocket};
use tradingview_rs::quote::QuoteValue;
use tradingview_rs::socket::DataServer;
type Result<T> = std::result::Result<T, tradingview_rs::error::Error>;

#[tokio::main]
async fn main() {
    dotenv().ok();
    tracing_subscriber::fmt::init();

    let auth_token = env::var("TV_AUTH_TOKEN").unwrap();

    let handlers = QuoteCallbackFn {
        data: Box::new(|data| Box::pin(on_data(data))),
        loaded: Box::new(|data| Box::pin(on_loaded(data))),
        error: Box::new(|data| Box::pin(on_error(data))),
    };

    let mut socket = WebSocket::build()
        .server(DataServer::ProData)
        .auth_token(auth_token)
        .connect(handlers)
        .await
        .unwrap();

    socket.create_session().await.unwrap();
    socket.set_fields().await.unwrap();

    socket
        .add_symbols(vec![
            "SP:SPX",
            "BINANCE:BTCUSDT",
            "BINANCE:ETHUSDT",
            "BITSTAMP:ETHUSD",
            "NASDAQ:TSLA",
            // "BINANCE:B",
        ])
        .await
        .unwrap();

    socket.subscribe().await;
}

async fn on_data(data: HashMap<String, QuoteValue>) -> Result<()> {
    data.iter().for_each(|(_, v)| {
        let json_string = serde_json::to_string(&v).unwrap();
        info!("{}", json_string);
    });
    Ok(())
}

async fn on_loaded(msg: Vec<serde_json::Value>) -> Result<()> {
    info!("Data: {:#?}", msg);
    Ok(())
}

async fn on_error(err: tradingview_rs::error::Error) -> Result<()> {
    error!("Error: {:#?}", err);
    Ok(())
}
