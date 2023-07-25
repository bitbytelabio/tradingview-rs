use tracing::debug;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    // let mut quote_socket = tradingview_rs::quote::websocket::Socket::new(
    //     tradingview_rs::socket::DataServer::Data,
    //     None,
    // )
    // .await;

    // quote_socket.read_message().await;

    use tradingview_rs::user::*;
    // let user = User::new().build();
    let user = User::new().credentials("aaa", "bbb").build().await;
    debug!("user: {:#?}", user);
}
