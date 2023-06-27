use crate::utils::protocol::{format_packet, parse_packet};
use serde_json::Value;
use std::io::{Read, Write};
use std::net::TcpStream;
use tungstenite::{client::IntoClientRequest, connect, stream::MaybeTlsStream, Message, WebSocket};
use url::Url;

trait Client {
    fn new() -> Self;
    fn send(&mut self, msg: &str);
    fn recv(&mut self) -> String;
}

// impl TVSocket {
//     pub fn new() -> Self {
//         let url = Url::parse("wss://data.tradingview.com/socket.io/websocket").unwrap();
//         let mut request = url.into_client_request().unwrap();
//         let headers = request.headers_mut();
//         headers.insert("Origin", "https://data.tradingview.com/".parse().unwrap());
//         let (socket, _) = connect(request).unwrap();
//         println!("WebSocket handshake has been successfully completed");
//         TVSocket { socket }
//     }
//     pub fn send() {}
// }
