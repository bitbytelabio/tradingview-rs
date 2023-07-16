use crate::auth::UserData;
use crate::utils::gen_session_id;
use crate::utils::protocol::{format_packet, parse_packet};
use crate::UA;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::error::Error;
use std::fmt;
use std::net::TcpStream;
use tracing::{debug, error, info};
use tungstenite::{client::IntoClientRequest, connect, stream::MaybeTlsStream, Message, WebSocket};
use url::Url;

pub struct Socket {
    pub socket_stream: WebSocket<MaybeTlsStream<TcpStream>>,
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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SocketMessage {
    pub m: String,
    pub p: Vec<Value>,
}

impl Socket {
    pub fn new(data_server: DataServer, user_data: Option<UserData>) -> Self {
        let server = data_server.to_string();
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
        match user_data {
            Some(user_data) => {
                let auth = SocketMessage {
                    m: "set_auth_token".to_string(),
                    p: vec![Value::String(user_data.auth_token)],
                };
                let msg = format_packet(auth);
                match socket.write_message(msg.clone()) {
                    Ok(_) => debug!("Message sent successfully: {}", msg.to_string()),
                    Err(e) => error!("Error sending message: {:?}", e),
                }
            }
            None => {
                let auth = SocketMessage {
                    m: "set_auth_token".to_string(),
                    p: vec![Value::String("unauthorized_user_token".to_string())],
                };
                let msg = format_packet(auth);
                match socket.write_message(msg.clone()) {
                    Ok(_) => debug!("Message sent successfully: {}", msg.to_string()),
                    Err(e) => error!("Error sending message: {:?}", e),
                }
            }
        };
        Socket {
            socket_stream: socket,
            messages: vec![],
        }
    }

    pub fn send<T>(&mut self, packet: T) -> Result<(), Box<dyn Error>>
    where
        T: Serialize,
    {
        self.messages.push(format_packet(packet));
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
                    let parsed_msg = parse_packet(&msg.to_string());
                    parsed_msg.into_iter().for_each(|x| {
                        if x.is_number() {
                            self.socket_stream.write_message(msg.clone()).unwrap();
                            debug!("Message sent successfully: {}", msg.to_string());
                        }
                        debug!("Message received: {:?}", x);
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
