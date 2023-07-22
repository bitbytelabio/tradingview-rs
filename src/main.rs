use tracing::warn;
use tradingview_rs::quote::ALL_QUOTE_FIELDS;
use tradingview_rs::socket::SocketMessage;
use tradingview_rs::user::User;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    // let message =
    //     tradingview_rs::socket::SocketMessage::new("quote_create_session".to_string(), new_vec);

    // // println!("{:?}", message);
    // let format_msg = tradingview_rs::utils::format_packet(message).unwrap();
    // println!("{:?}", format_msg.to_string());

    // warn!(
    //     "{:?}",
    //     tradingview_rs::quote::websocket::Socket::set_quote_fields("qc_assdfdfgffds")
    // )

    let mut quote_socket = tradingview_rs::quote::websocket::Socket::new(
        tradingview_rs::socket::DataServer::Data,
        None,
    );

    quote_socket.read_message();
}
