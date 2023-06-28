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
    messages: Vec<Message>,
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

#[derive(Serialize)]
struct SocketMessage {
    m: Value,
    p: Value,
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
        match user_data {
            Some(user_data) => {
                let auth = SocketMessage {
                    m: Value::String("set_auth_token".to_string()),
                    p: Value::Array(vec![Value::String(user_data.auth_token)]),
                };
                let msg = format_packet(auth);
                match socket.write_message(msg.clone()) {
                    Ok(_) => debug!("Message sent successfully: {}", msg.to_string()),
                    Err(e) => error!("Error sending message: {:?}", e),
                }
            }
            None => {
                let auth = SocketMessage {
                    m: Value::String("set_auth_token".to_string()),
                    p: Value::Array(vec![Value::String("unauthorized_user_token".to_string())]),
                };
                let msg = format_packet(auth);
                match socket.write_message(msg.clone()) {
                    Ok(_) => debug!("Message sent successfully: {}", msg.to_string()),
                    Err(e) => error!("Error sending message: {:?}", e),
                }
            }
        };
        Socket {
            socket,
            messages: vec![],
        }
    }

    pub fn send<T>(&mut self, packet: T) -> Result<(), Box<dyn Error>>
    where
        T: Serialize,
    {
        let msg = serde_json::to_value(packet)?;
        let msg = format_packet(msg);
        self.messages.push(msg);
        self.send_queue();
        Ok(())
    }

    fn send_queue(&mut self) {
        while self.socket.can_read() && self.socket.can_write() && self.messages.len() > 0 {
            let msg = self.messages.remove(0);
            match self.socket.write_message(msg.clone()) {
                Ok(_) => debug!("Message sent successfully: {}", msg.to_string()),
                Err(e) => error!("Error sending message: {:?}", e),
            }
        }
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
