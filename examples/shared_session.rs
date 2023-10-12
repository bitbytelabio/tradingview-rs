use dotenv::dotenv;
use std::{ collections::HashMap, env, sync::Arc };
use tokio::sync::Mutex;
use tracing::{ error, info };
use tradingview::{
    chart::{ session::{ ChartCallbackFn, WebSocket as ChartSocket }, ChartOptions, ChartSeries },
    models::Interval,
    quote::{ session::{ QuoteCallbackFn, WebSocket as QuoteSocket }, QuoteValue },
    socket::DataServer,
    socket::SocketSession,
    user::User,
};

#[tokio::main]
async fn main() {
    dotenv().ok();
    tracing_subscriber::fmt::init();

    let session = env::var("TV_SESSION").unwrap();
    let signature = env::var("TV_SIGNATURE").unwrap();

    let user = User::build().session(&session, &signature).get().await.unwrap();

    let handlers = ChartCallbackFn {
        on_chart_data: Box::new(|data| Box::pin(on_chart_data(data))),
        on_symbol_resolved: Box::new(|data| Box::pin(on_symbol_resolved(data))),
        on_series_completed: Box::new(|data| Box::pin(on_series_completed(data))),
    };

    let socket_session = SocketSession::new(DataServer::ProData, user.auth_token).await.unwrap();

    let mut chart_socket = ChartSocket::build()
        .socket(socket_session.clone())
        .connect(handlers).await
        .unwrap();

    let mut quote_socket = QuoteSocket::build()
        .socket(socket_session.clone())
        .connect(QuoteCallbackFn {
            data: Box::new(|data| Box::pin(on_data(data))),
            loaded: Box::new(|data| Box::pin(on_loaded(data))),
            error: Box::new(|data| Box::pin(on_error(data))),
        }).await
        .unwrap();

    quote_socket.create_session().await.unwrap();
    quote_socket.set_fields().await.unwrap();
    quote_socket
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

    chart_socket
        .set_market(ChartOptions {
            symbol: "BINANCE:BTCUSDT".to_string(),
            resolution: Interval::FourHours,
            bar_count: 5,
            range: Some("60M".to_string()),
            ..Default::default()
        }).await
        .unwrap();

    let chart_socket = Arc::new(Mutex::new(chart_socket));
    let quote_socket = Arc::new(Mutex::new(quote_socket));

    tokio::spawn(async move { chart_socket.clone().lock().await.subscribe().await });
    tokio::spawn(async move { quote_socket.clone().lock().await.subscribe().await });

    loop {
        // wait for receiving data
        // dummy loop
        std::thread::sleep(std::time::Duration::from_secs(60));
        let new_token = User::build().session(&session, &signature).get().await.unwrap().auth_token;

        socket_session.clone().update_token(&new_token).await.unwrap();
    }
}

async fn on_chart_data(data: ChartSeries) {
    info!("on_chart_data: {:?}", data);
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

async fn on_symbol_resolved(data: tradingview::chart::SymbolInfo) {
    info!("on_symbol_resolved: {:?}", data);
}

async fn on_series_completed(data: tradingview::chart::SeriesCompletedMessage) {
    info!("on_series_completed: {:?}", data);
}
