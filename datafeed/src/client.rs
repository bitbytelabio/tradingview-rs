use crate::auth::UserData;
use crate::utils::gen_session_id;
use crate::utils::protocol::{format_packet, parse_packet};
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

pub trait SocketClient {
    type QuoteSession: fmt::Display;
    type ChartSession: fmt::Display;
    type ReplaySession: fmt::Display;
    fn quote_create_session(&mut self) -> Self::QuoteSession;
    fn quote_set_fields(&mut self, session: Self::QuoteSession);
    fn quote_add_symbols(&mut self, session: Self::QuoteSession, symbol: String);
    fn chart_create_session(&mut self) -> Self::ChartSession;
    fn chart_set_fields(&mut self, session: Self::ChartSession);
    fn chart_add_symbols(&mut self, session: Self::ChartSession, symbol: String);
    fn replay_create_session(&mut self) -> Self::ReplaySession;
    fn replay_set_fields(&mut self, session: Self::ReplaySession);
    fn replay_add_symbols(&mut self, session: Self::ReplaySession, symbol: String);
    fn new(data_server: DataServer, user_data: Option<UserData>) -> Self;
    fn send<T>(&mut self, packet: T) -> Result<(), Box<dyn Error>>
    where
        T: Serialize;
    fn read_message(&mut self);
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
                    debug!("Message received: {}", msg.to_string());
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
