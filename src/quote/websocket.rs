use std::collections::VecDeque;

use super::session;
use crate::socket::SocketMessage;
use crate::utils::{format_packet, parse_packet};
use crate::UA;

use tracing::{debug, error, info, warn};
use url::Url;

use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use tokio::net::TcpStream;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, protocol::Message},
    MaybeTlsStream, WebSocketStream,
};

pub struct Socket {
    pub write: SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>,
    pub read: SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    session: String,
    messages: VecDeque<Message>,
    callbacks: Vec<Option<Box<dyn FnMut(SocketMessage) + Send + Sync + 'static>>>,
}

impl Socket {
    pub fn set_quote_fields(session: &str) -> SocketMessage {
        let mut p_vec = vec![session.to_string()];
        super::ALL_QUOTE_FIELDS.iter().for_each(|field| {
            p_vec.push(field.to_string());
        });

        SocketMessage::new("quote_set_fields", p_vec)
    }

    pub async fn new(server: crate::socket::DataServer, auth_token: Option<String>) -> Self {
        let server = server.to_string();
        let url = Url::parse("wss://data.tradingview.com/socket.io/websocket").unwrap();
        let mut request = url.into_client_request().unwrap();
        let headers = request.headers_mut();
        headers.insert("Origin", "https://www.tradingview.com/".parse().unwrap());
        headers.insert("User-Agent", UA.parse().unwrap());
        let mut socket = match connect_async(request).await {
            Ok((socket, _)) => socket,
            Err(e) => panic!("Error during handshake: {}", e),
        };
        info!("WebSocket handshake has been successfully completed");

        let (mut write, mut read) = socket.split();

        // match auth_token {
        //     Some(auth_token) => {
        //         let auth = SocketMessage::new("set_auth_token", vec![auth_token]);

        //         let msg = format_packet(auth).unwrap();

        //         match socket.write_message(msg.clone()) {
        //             Ok(_) => debug!("Message sent successfully: {}", msg.to_string()),
        //             Err(e) => error!("Error sending message: {:?}", e),
        //         }
        //     }
        //     None => {
        //         let auth = SocketMessage::new("set_auth_token", vec!["unauthorized_user_token"]);
        //         let msg = format_packet(auth).unwrap();
        //         match socket.write_message(msg.clone()) {
        //             Ok(_) => debug!("Message sent successfully: {}", msg.to_string()),
        //             Err(e) => error!("Error sending message: {:?}", e),
        //         }
        //     }
        // };

        let session = crate::utils::gen_session_id("qs");

        let messages = vec![
            format_packet(SocketMessage::new(
                "set_auth_token",
                vec!["unauthorized_user_token"],
            ))
            .unwrap(),
            format_packet(SocketMessage::new("quote_create_session", vec![&session])).unwrap(),
            format_packet(Self::set_quote_fields(&session)).unwrap(),
            format_packet(SocketMessage::new(
                "quote_add_symbols",
                vec![session.clone(), "BINANCE:BTCUSDT".to_string()],
            ))
            .unwrap(),
        ];

        for msg in messages {
            match write.send(msg).await {
                Ok(_) => debug!("Message sent successfully"),
                Err(e) => error!("Error sending message: {:?}", e),
            }
        }

        Socket {
            // SocketStream: socket,
            write: write,
            read: read,
            messages: vec![],
        }
    }

    // pub fn send<T>(&mut self, packet: T) -> Result<(), Box<dyn Error>>
    // where
    //     T: Serialize,
    // {
    //     self.messages.push(format_packet(packet).unwrap());
    //     self.send_queue();
    //     Ok(())
    // }

    // fn send_queue(&mut self) {
    //     while self.SocketStream.can_read()
    //         && self.SocketStream.can_write()
    //         && self.messages.len() > 0
    //     {
    //         let msg = self.messages.remove(0);
    //         match self.SocketStream.send(msg.clone()) {
    //             Ok(_) => {
    //                 debug!("Message sent successfully: {}", msg.to_string());
    //             }
    //             Err(e) => {
    //                 error!("Error sending message: {:?}", e);
    //                 self.messages.push(msg);
    //             }
    //         }
    //     }
    // }

    pub async fn read_message(&mut self) {
        while let Some(result) = self.read.next().await {
            match result {
                Ok(msg) => {
                    let parsed_msg = parse_packet(&msg.to_string()).unwrap();
                    for x in parsed_msg {
                        if x.is_number() {
                            let y = self.write.send(msg.clone()).await;
                            match y {
                                Ok(_) => {
                                    debug!("Message sent successfully: {:#?}", msg.to_string())
                                }
                                Err(e) => error!("Error sending message: {:#?}", e),
                            }
                        } else if x["m"].is_string() && x["m"] == "qsd" {
                            let quote_data =
                                serde_json::from_value::<crate::model::Quote>(x["p"][1].clone())
                                    .unwrap();
                            info!("Quote data: {:#?}", quote_data);
                        }
                        debug!("Message received: {:#?}", x);
                    }
                }
                Err(e) => {
                    error!("Error reading message: {:#?}", e);
                }
            }
        }
    }
}
