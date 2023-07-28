use tracing::debug;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let mut socket = tradingview_rs::quote::websocket::QuoteSocket::new(
        tradingview_rs::socket::DataServer::Data,
    )
    // .quote_fields(vec!["lp".to_string()])
    .build()
    .await
    .unwrap();

    socket
        .quote_add_symbols(vec![
            "BINANCE:BTCUSDT".to_string(),
            "BINANCE:ETHUSDT".to_string(),
        ])
        .await
        .unwrap();

    socket.load().await;

    // use tradingview_rs::user::*;
    // // let user = User::new().build();
    // let user = User::new()
    //     // .credentials("", "")
    //     // .pro(true)
    //     .session(
    //         "2endn5wa0rw9gmu8bewxc6ymqbxhrbw2",
    //         "v1:+MY3Ea/emfmC56ZMMu2Q5PO+kW3ZB36QxiW2U6nEhIw=",
    //     )
    //     .build()
    //     .await;
    // debug!("user: {:#?}", user);
}
