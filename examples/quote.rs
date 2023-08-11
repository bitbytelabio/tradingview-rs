use std::collections::HashMap;
use std::env;

use tracing::{error, info};
use tradingview_rs::error::TradingViewError;
use tradingview_rs::quote::session::{QuoteCallbackFn, WebSocket};
use tradingview_rs::quote::QuoteValue;
use tradingview_rs::socket::DataServer;
use tradingview_rs::user::User;
type Result<T> = std::result::Result<T, tradingview_rs::error::Error>;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let session = env::var("TV_SESSION").unwrap();
    let signature = env::var("TV_SIGNATURE").unwrap();

    let user = User::build()
        .session(&session, &signature)
        .get()
        .await
        .unwrap();

    let handlers = QuoteCallbackFn {
        data: Box::new(on_data),
        loaded: Box::new(on_loaded),
        error: Box::new(on_error),
    };

    let mut socket = WebSocket::build()
        .server(DataServer::ProData)
        .auth_token(user.auth_token)
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

fn on_data(data: HashMap<String, QuoteValue>) -> Result<()> {
    data.iter().for_each(|(_, v)| {
        let json_string = serde_json::to_string(&v).unwrap();
        info!("{}", json_string);
    });
    Ok(())
}

fn on_loaded(msg: Vec<serde_json::Value>) -> Result<()> {
    info!("Data: {:#?}", msg);
    Ok(())
}

fn on_error(err: TradingViewError) -> Result<()> {
    error!("Error: {:#?}", err);
    Ok(())
}
