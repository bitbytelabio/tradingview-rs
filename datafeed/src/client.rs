use crate::auth::UserData;
use crate::utils::protocol::{format_packet, parse_packet};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::net::TcpStream;
use tracing::{debug, error, info};
use tungstenite::{client::IntoClientRequest, connect, stream::MaybeTlsStream, Message, WebSocket};
use url::Url;

pub struct Socket {
    pub socket: WebSocket<MaybeTlsStream<TcpStream>>,
}

pub enum DataServer {
    Data,
    ProData,
    WidgetData,
}

impl fmt::Display for DataServer {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            DataServer::Data => write!(f, "data"),
            DataServer::ProData => write!(f, "prodata"),
            DataServer::WidgetData => write!(f, "widgetdata"),
        }
    }
}

impl Socket {
    pub fn new(data_server: DataServer, user_data: Option<UserData>) -> Self {
        let server = data_server.to_string();
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

        let mut msg: HashMap<String, Value> = HashMap::new();
        msg.insert("m".to_string(), Value::String("set_auth_token".to_string()));
        match user_data {
            Some(user_data) => {
                msg.insert(
                    "p".to_string(),
                    Value::Array(vec![Value::String(user_data.auth_token)]),
                );
            }
            None => {
                msg.insert(
                    "p".to_string(),
                    Value::Array(vec![Value::String("unauthorized_user_token".to_string())]),
                );
            }
        }

        let msg = format_packet(msg);
        socket.write_message(msg).unwrap();
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
