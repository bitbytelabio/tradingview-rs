use dotenv::dotenv;
use std::collections::HashMap;
use std::env;
use tracing::{ error, info };
use tradingview::quote::session::{ QuoteCallbackFn, WebSocket };
use tradingview::quote::QuoteValue;
use tradingview::socket::DataServer;

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
        .connect(handlers).await
        .unwrap();

    socket.create_session().await.unwrap();
    socket.set_fields().await.unwrap();

    socket
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

    socket.subscribe().await;
}

async fn on_data(data: HashMap<String, QuoteValue>) {
    data.iter().for_each(|(_, v)| {
        let json_string = serde_json::to_string(&v).unwrap();
        info!("{}", json_string);
    });
}

async fn on_loaded(msg: Vec<serde_json::Value>) {
    info!("Data: {:#?}", msg);
}

async fn on_error(err: tradingview::error::Error) {
    error!("Error: {:#?}", err);
}
