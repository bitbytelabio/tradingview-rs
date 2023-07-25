use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct SocketMessage {
    pub m: Value,
    pub p: Value,
}

impl SocketMessage {
    pub fn new<M, P>(m: M, p: Vec<P>) -> Self
    where
        M: Serialize,
        P: Serialize,
    {
        let m = serde_json::to_value(m).expect("Failed to serialize Socket Message");
        let p = serde_json::to_value(p).expect("Failed to serialize Socket Message");
        SocketMessage { m, p }
    }
}

pub enum DataServer {
    Data,
    ProData,
    WidgetData,
}

impl std::fmt::Display for DataServer {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            DataServer::Data => write!(f, "data"),
            DataServer::ProData => write!(f, "prodata"),
            DataServer::WidgetData => write!(f, "widgetdata"),
        }
    }
}

pub enum WebSocketEvent {
    Message(String),
    Error(String),
    Connected,
    Disconnected,
    Logged,
    Ping,
    Data,
    Event,
}

pub enum Event {}

// pub trait Socket {
//     fn new<Callback>(handler: Callback) -> Self
//     where
//         Callback: FnMut(WebSocketEvent) + Send + Sync + 'static;

//     fn connect(&mut self, server: DataServer) -> Result<(), Box<dyn std::error::Error>>;

//     fn disconnect(&mut self) -> Result<(), Box<dyn std::error::Error>>;

//     fn send<T>(&mut self, message: T) -> Result<(), Box<dyn std::error::Error>>;

//     fn send_queue(&mut self) -> Result<(), Box<dyn std::error::Error>>;

//     fn handle_msg(&mut self, msg: &str) -> Result<(), Box<dyn std::error::Error>>;

//     fn handle_event(&mut self, event: WebSocketEvent) -> Result<(), Box<dyn std::error::Error>>;

//     fn handle_error(&mut self, error: String) -> Result<(), Box<dyn std::error::Error>>;
// }
