use crate::auth::UserData;
use crate::utils::protocol::{format_packet, parse_packet, Packet};
use serde_json::Value;
use std::error::Error;
use std::net::TcpStream;
use tracing::{error, info};
use tungstenite::{client::IntoClientRequest, connect, stream::MaybeTlsStream, Message, WebSocket};
use url::Url;

pub struct Socket {
    socket: WebSocket<MaybeTlsStream<TcpStream>>,
}

impl Socket {
    pub fn new(pro: bool, user_data: UserData) -> Self {
        let server = if pro { "prodata" } else { "data" };
        let url = Url::parse(&format!(
            "wss://{}.tradingview.com/socket.io/websocket",
            server
        ))
        .unwrap();
        let mut request = url.into_client_request().unwrap();
        let headers = request.headers_mut();
        headers.insert("Origin", "https://data.tradingview.com/".parse().unwrap());
        let mut socket = match connect(request) {
            Ok((socket, _)) => socket,
            Err(e) => panic!("Error during handshake: {}", e),
        };
        info!("WebSocket handshake has been successfully completed");

        let result = socket.write_message(Message::Text(
            format_packet(&Packet {
                p: Value::String("set_auth_token".to_string()),
                m: Value::Array(vec![Value::String("unauthorized_user_token".to_string())]),
            })
            .unwrap(),
        ));

        dbg!(result);

        Socket { socket }
    }
    pub fn send(&mut self, packet: &Packet) -> Result<(), Box<dyn Error>> {
        let msg = match format_packet(packet) {
            Ok(msg) => msg,
            Err(e) => {
                error!("Error formatting packet: {}", e);
                Err("Error formatting packet")?
            }
        };
        self.socket.write_message(Message::Text(msg)).unwrap();
        Ok(())
    }
}
