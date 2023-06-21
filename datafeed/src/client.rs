use crate::utils::protocol::{format_ws_packet, parse_ws_packet};
use serde_json::Value;
use std::io::{Read, Write};
use std::net::TcpStream;
use tungstenite::{client::IntoClientRequest, connect, stream::MaybeTlsStream, Message, WebSocket};
use url::Url;
struct TVSocket {
    socket: WebSocket<MaybeTlsStream<TcpStream>>,
}

impl TVSocket {
    pub fn new() -> Self {
        let url = Url::parse("wss://data.tradingview.com/socket.io/websocket").unwrap();
        let mut request = url.into_client_request().unwrap();
        let headers = request.headers_mut();
        headers.insert("Origin", "https://data.tradingview.com/".parse().unwrap());
        let (socket, _) = connect(request).unwrap();
        println!("WebSocket handshake has been successfully completed");
        TVSocket { socket }
    }
    pub fn send() {}
}
