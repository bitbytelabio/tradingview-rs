use crate::socket::SocketMessage;
use crate::utils::{format_packet, parse_packet};
use crate::UA;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::error::Error;
use std::net::TcpStream;
use tracing::{debug, error, info, warn};
use tungstenite::{client::IntoClientRequest, connect, stream::MaybeTlsStream, Message, WebSocket};
use url::Url;

use super::session;
pub struct Socket {
    pub socket_stream: WebSocket<MaybeTlsStream<TcpStream>>,
    messages: Vec<Message>,
}

impl Socket {
    pub fn set_quote_fields(session: &str) -> SocketMessage {
        let mut p_vec = vec![session.to_string()];
        super::ALL_QUOTE_FIELDS.iter().for_each(|field| {
            p_vec.push(field.to_string());
        });

        SocketMessage::new("quote_set_fields", p_vec)
    }

    pub fn new(server: crate::socket::DataServer, auth_token: Option<String>) -> Self {
        let server = server.to_string();
        let url = Url::parse("wss://data.tradingview.com/socket.io/websocket").unwrap();
        let mut request = url.into_client_request().unwrap();
        let headers = request.headers_mut();
        headers.insert("Origin", "https://www.tradingview.com/".parse().unwrap());
        headers.insert("User-Agent", UA.parse().unwrap());
        let mut socket = match connect(request) {
            Ok((socket, _)) => socket,
            Err(e) => panic!("Error during handshake: {}", e),
        };
        info!("WebSocket handshake has been successfully completed");
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

        messages
            .iter()
            .for_each(|msg| match socket.write_message(msg.clone()) {
                Ok(_) => debug!("Message sent successfully: {:?}", msg.to_string()),
                Err(e) => error!("Error sending message: {:?}", e),
            });

        Socket {
            socket_stream: socket,
            messages: vec![],
        }
    }

    pub fn send<T>(&mut self, packet: T) -> Result<(), Box<dyn Error>>
    where
        T: Serialize,
    {
        self.messages.push(format_packet(packet).unwrap());
        self.send_queue();
        Ok(())
    }

    fn send_queue(&mut self) {
        while self.socket_stream.can_read()
            && self.socket_stream.can_write()
            && self.messages.len() > 0
        {
            let msg = self.messages.remove(0);
            match self.socket_stream.write_message(msg.clone()) {
                Ok(_) => {
                    debug!("Message sent successfully: {}", msg.to_string());
                }
                Err(e) => {
                    error!("Error sending message: {:?}", e);
                    self.messages.push(msg);
                }
            }
        }
    }

    pub fn read_message(&mut self) {
        loop {
            let result = self.socket_stream.read_message();
            match result {
                Ok(msg) => {
                    let parsed_msg = parse_packet(&msg.to_string()).unwrap();
                    parsed_msg.into_iter().for_each(|x| {
                        if x.is_number() {
                            self.socket_stream.write_message(msg.clone()).unwrap();
                            debug!("Message sent successfully: {}", msg.to_string());
                        } else if x["m"].is_string() && x["m"] == "qsd" {
                            let quote_data =
                                serde_json::from_value::<crate::model::Quote>(x["p"][1].clone())
                                    .unwrap();
                            info!("Quote data: {:?}", quote_data);
                        }
                        warn!("Message received: {:?}", x);
                    });
                }
                Err(e) => {
                    error!("Error reading message: {:?}", e);
                    break;
                }
            }
        }
    }
}
