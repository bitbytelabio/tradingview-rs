use tradingview_rs::quote::ALL_QUOTE_FIELDS;
use tradingview_rs::socket::SocketMessage;
use tradingview_rs::user::User;
use tradingview_rs::utils::{format_packet, parse_packet};
use tradingview_rs::UA;

use futures_util::{SinkExt, StreamExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use tracing::{debug, error, info, warn};

use tungstenite::client::IntoClientRequest;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let url = url::Url::parse("wss://data.tradingview.com/socket.io/websocket").unwrap();
    let mut request = url.into_client_request().unwrap();
    let headers = request.headers_mut();
    headers.insert("Origin", "https://www.tradingview.com/".parse().unwrap());
    headers.insert("User-Agent", UA.parse().unwrap());

    let (mut ws_stream, _) = connect_async(request).await.expect("Failed to connect");
    println!("WebSocket handshake has been successfully completed");

    let (mut write, read) = ws_stream.split();

    let session = tradingview_rs::utils::gen_session_id("qs");

    let messages = vec![
        format_packet(SocketMessage::new(
            "set_auth_token",
            vec!["unauthorized_user_token"],
        ))
        .unwrap(),
        format_packet(SocketMessage::new("quote_create_session", vec![&session])).unwrap(),
        format_packet(tradingview_rs::quote::websocket::Socket::set_quote_fields(
            &session,
        ))
        .unwrap(),
        format_packet(SocketMessage::new(
            "quote_add_symbols",
            vec![session.clone(), "BINANCE:BTCUSDT".to_string()],
        ))
        .unwrap(),
    ];

    // ws_stream
    //     .for_each(|msg| async {
    //         match msg {
    //             Ok(msg) => {
    //                 let data = msg.into_text().unwrap();
    //                 let parsed_msg = parse_packet(&data).unwrap();
    //                 for x in parsed_msg {
    //                     if x.is_number() {
    //                         // write.send(Message::Text("1".to_string())).await.unwrap();
    //                         // debug!("Received ping message: {}", data);
    //                     } else if x["m"].is_string() && x["m"] == "qsd" {
    //                         let quote_data =
    //                             serde_json::from_value::<tradingview_rs::model::Quote>(
    //                                 x["p"][1].clone(),
    //                             )
    //                             .unwrap();
    //                         info!("Quote data: {:?}", quote_data);
    //                     }
    //                     debug!("Message received: {:?}", x);
    //                 }
    //             }
    //             Err(e) => error!("Error receiving message: {:?}", e),
    //         }
    //     })
    //     .await;

    for msg in messages {
        match write.send(msg).await {
            Ok(_) => debug!("Message sent successfully."),
            Err(e) => error!("Error sending message: {:?}", e),
        }
    }

    read.for_each(|message| async {
        let data = message.unwrap().into_text().unwrap();
        let parsed_msg = parse_packet(&data).unwrap();
        for x in parsed_msg {
            if x.is_number() {
                // write.send(Message::Text("1".to_string())).await.unwrap();
                debug!("Received ping message: {}", data);
            } else if x["m"].is_string() && x["m"] == "qsd" {
                let quote_data =
                    serde_json::from_value::<tradingview_rs::model::Quote>(x["p"][1].clone())
                        .unwrap();
                info!("Quote data: {:?}", quote_data);
            }
            debug!("Message received: {:?}", x);
        }
    })
    .await;
}
