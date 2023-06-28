use crate::auth::UserData;
use crate::utils::protocol::{format_packet, parse_packet};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::error::Error;
use std::net::TcpStream;
use tracing::{error, info};
use tungstenite::{client::IntoClientRequest, connect, stream::MaybeTlsStream, Message, WebSocket};
use url::Url;

pub struct Socket {
    pub socket: WebSocket<MaybeTlsStream<TcpStream>>,
}

impl Socket {
    pub fn new(pro: bool) -> Self {
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

        let mut msg: HashMap<String, Vec<String>> = HashMap::new();
        // msg.add("set_auth_token".to_string(), vec![UserData::get_token()]);

        Socket { socket }
    }
    pub fn send<T>(&mut self, packet: T) -> Result<(), Box<dyn Error>>
    where
        T: Serialize,
    {
        let msg = format_packet(packet);
        self.socket.write_message(msg).unwrap();
        Ok(())
    }
    pub fn read_message(&mut self) {
        loop {
            let result = self.socket.read_message();
            match result {
                Ok(msg) => {
                    info!("{}", msg.to_text().unwrap());
                }
                Err(e) => {
                    error!("Error reading message: {:?}", e);
                    break;
                }
            }
        }
    }
}
