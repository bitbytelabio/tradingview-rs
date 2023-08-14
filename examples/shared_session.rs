use std::{collections::HashMap, env, sync::Arc};

use tokio::sync::Mutex;
use tracing::{error, info};
use tradingview_rs::error::TradingViewError;
use tradingview_rs::{
    chart::{
        session::{ChartCallbackFn, Options, WebSocket as ChartSocket},
        ChartSeries,
    },
    models::Interval,
    quote::{
        session::{QuoteCallbackFn, WebSocket as QuoteSocket},
        QuoteValue,
    },
    socket::DataServer,
    socket::SocketSession,
    user::User,
};

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

    let handlers = ChartCallbackFn {
        on_chart_data: Box::new(|data| Box::pin(on_chart_data(data))),
        on_symbol_resolve: Box::new(|data| Box::pin(on_symbol_resolve(data))),
    };

    let session = SocketSession::new(DataServer::ProData, user.auth_token)
        .await
        .unwrap();

    let mut chart_socket = ChartSocket::build()
        .socket(session.clone())
        .connect(handlers)
        .await
        .unwrap();

    let mut quote_socket = QuoteSocket::build()
        .socket(session)
        .connect(QuoteCallbackFn {
            data: Box::new(on_data),
            loaded: Box::new(on_loaded),
            error: Box::new(on_error),
        })
        .await
        .unwrap();

    quote_socket.create_session().await.unwrap();
    quote_socket.set_fields().await.unwrap();
    quote_socket
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

    chart_socket
        .set_market(
            "BINANCE:ETHUSDT",
            Options {
                resolution: Interval::FourHours,
                bar_count: 5,
                range: Some("60M".to_string()),
                ..Default::default()
            },
        )
        .await
        .unwrap();

    let chart_socket = Arc::new(Mutex::new(chart_socket));
    let quote_socket = Arc::new(Mutex::new(quote_socket));

    tokio::spawn(async move { chart_socket.clone().lock().await.subscribe().await });
    tokio::spawn(async move { quote_socket.clone().lock().await.subscribe().await });

    loop {
        // wait for receiving data
    }
}

async fn on_chart_data(data: ChartSeries) -> Result<(), tradingview_rs::error::Error> {
    info!("on_chart_data: {:?}", data);
    Ok(())
}

fn on_data(data: HashMap<String, QuoteValue>) -> Result<(), tradingview_rs::error::Error> {
    data.iter().for_each(|(_, v)| {
        let json_string = serde_json::to_string(&v).unwrap();
        info!("{}", json_string);
    });
    Ok(())
}

fn on_loaded(msg: Vec<serde_json::Value>) -> Result<(), tradingview_rs::error::Error> {
    info!("Data: {:#?}", msg);
    Ok(())
}

fn on_error(err: TradingViewError) -> Result<(), tradingview_rs::error::Error> {
    error!("Error: {:#?}", err);
    Ok(())
}

async fn on_symbol_resolve(
    data: tradingview_rs::chart::SymbolInfo,
) -> Result<(), tradingview_rs::error::Error> {
    info!("on_symbol_resolve: {:?}", data);
    Ok(())
}
