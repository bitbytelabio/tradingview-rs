use tradingview_rs::quote::ALL_QUOTE_FIELDS;
use tradingview_rs::socket::SocketMessage;
use tradingview_rs::user::User;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let session = tradingview_rs::utils::gen_session_id("qs");
    let mut new_vec: Vec<String> = vec![session];
    new_vec.extend_from_slice(&ALL_QUOTE_FIELDS);

    let message =
        tradingview_rs::socket::SocketMessage::new("quote_create_session".to_string(), new_vec);

    // println!("{:?}", message);
    let format_msg = tradingview_rs::utils::format_packet(message).unwrap();
    println!("{:?}", format_msg.to_string());
}
