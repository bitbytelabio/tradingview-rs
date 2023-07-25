use crate::{
    errors::SocketError,
    socket::{DataServer, SocketMessage, WebSocketEvent},
    utils::{format_packet, gen_session_id, parse_packet},
    UA,
};
use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use serde::{Serialize, Serializer};
use std::collections::{vec_deque, VecDeque};
use tokio::net::TcpStream;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, protocol::Message},
    MaybeTlsStream, WebSocketStream,
};
use tracing::{debug, error, info, warn};
use url::Url;

pub struct QuoteSocket {
    pub write: SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>,
    pub read: SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    session: String,
    messages: VecDeque<Message>,
    // callbacks: Option<Box<dyn FnMut(SocketMessage) + Send + Sync + 'static>>,
}

pub struct QuoteSocketBuilder {
    server: DataServer,
    auth_token: Option<String>,
}

impl QuoteSocketBuilder {}

impl QuoteSocket {
    fn set_quote_fields(session: &str) -> SocketMessage {
        let mut params = vec![session.to_string()];
        super::ALL_QUOTE_FIELDS.iter().for_each(|field| {
            params.push(field.to_string());
        });
        SocketMessage::new("quote_set_fields", params)
    }

    pub async fn connect(
        &mut self,
        server: crate::socket::DataServer,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let url = Url::parse(&format!(
            "wss://{server}.tradingview.com/socket.io/websocket",
            server = server.to_string()
        ))
        .unwrap();

        let mut request = url.into_client_request().unwrap();
        let headers = request.headers_mut();
        headers.insert("Origin", "https://www.tradingview.com/".parse().unwrap());
        headers.insert("User-Agent", UA.parse().unwrap());

        let socket = match connect_async(request).await {
            Ok((socket, _)) => socket,
            Err(e) => return Err(Box::new(e)),
        };

        info!("WebSocket handshake has been successfully completed");

        let (write, read) = socket.split();

        self.write = write;
        self.read = read;
        self.session = gen_session_id("qs");

        Ok(())
    }

    async fn send<T>(&mut self, message: T) -> Result<(), Box<dyn std::error::Error>>
    where
        T: Serialize,
    {
        Ok(())
    }

    async fn send_queue(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    async fn close(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    async fn on_connected(&mut self) {
        todo!()
    }

    async fn on_ping(&mut self) {
        todo!()
    }

    async fn on_data(&mut self) {
        todo!()
    }

    async fn on_event(&mut self) {
        todo!()
    }

    async fn on_error(&mut self) {
        todo!()
    }

    async fn on_disconnected(&mut self) {
        todo!()
    }

    async fn on_logged(&mut self) {
        todo!()
    }
}
