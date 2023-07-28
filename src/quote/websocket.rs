use crate::{
    prelude::*,
    socket::{DataServer, SocketMessage, WebSocketEvent},
    utils::{format_packet, gen_session_id, parse_packet},
    UA,
};
use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use serde::Serialize;
use std::collections::VecDeque;
use tokio::net::TcpStream;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, protocol::Message},
    MaybeTlsStream, WebSocketStream,
};
use tracing::{debug, error, info, warn};
use url::Url;

#[derive(Debug)]
pub struct QuoteSocket {
    write: SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>,
    read: SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    session: String,
    messages: VecDeque<Message>,
    auth_token: String,
    // callbacks: Option<Box<dyn FnMut(SocketMessage) + Send + Sync + 'static>>,
}

pub struct Callback {}

pub struct QuoteSocketBuilder {
    server: DataServer,
    auth_token: Option<String>,
    quote_fields: Option<Vec<String>>,
}

impl QuoteSocketBuilder {
    pub fn auth_token(&mut self, auth_token: String) -> &mut Self {
        self.auth_token = Some(auth_token);
        self
    }

    pub fn quote_fields(&mut self, quote_fields: Vec<String>) -> &mut Self {
        self.quote_fields = Some(quote_fields);
        self
    }

    fn init_messages(&self, session: &str, auth_token: &str) -> Result<VecDeque<Message>> {
        let quote_fields: Vec<String> = match self.quote_fields.clone() {
            Some(fields) => {
                let mut quote_fields = vec![session.clone().to_string()];
                quote_fields.extend(fields);
                quote_fields
            }
            None => {
                let mut quote_fields = vec![session.clone().to_string()];
                quote_fields.extend(crate::quote::ALL_QUOTE_FIELDS.clone());
                quote_fields
            }
        };

        Ok(VecDeque::from(vec![
            SocketMessage::new("set_auth_token", vec![auth_token]).to_message()?,
            SocketMessage::new("quote_create_session", vec![session]).to_message()?,
            SocketMessage::new("quote_set_fields", quote_fields).to_message()?,
        ]))
    }

    pub async fn build(&mut self) -> Result<QuoteSocket> {
        let url = Url::parse(&format!(
            "wss://{server}.tradingview.com/socket.io/websocket",
            server = self.server.to_string()
        ))
        .unwrap();

        let mut request = url.into_client_request().unwrap();
        let headers = request.headers_mut();
        headers.insert("Origin", "https://www.tradingview.com/".parse().unwrap());
        headers.insert("User-Agent", UA.parse().unwrap());

        let (socket, _) = connect_async(request).await?;

        info!("WebSocket handshake has been successfully completed");

        let (write, read) = socket.split();

        let auth_token = match self.auth_token.clone() {
            Some(token) => token,
            None => "unauthorized_user_token".to_string(),
        };

        let session = gen_session_id("qs");

        let messages = self.init_messages(&session, &auth_token)?;

        Ok(QuoteSocket {
            write,
            read,
            session,
            messages,
            auth_token,
        })
    }
}

impl QuoteSocket {
    pub fn new(server: DataServer) -> QuoteSocketBuilder {
        return QuoteSocketBuilder {
            server: server,
            auth_token: None,
            quote_fields: None,
        };
    }

    pub async fn set_local(&mut self, local: Vec<String>) -> Result<()> {
        self.send("set_local", local).await?;
        Ok(())
    }

    pub async fn switch_timezone(&mut self, timezone: &str) -> Result<()> {
        self.send("switch_timezone", vec![timezone]).await?;
        Ok(())
    }

    pub async fn quote_add_symbols(&mut self, symbols: Vec<String>) -> Result<()> {
        let mut payload = vec![self.session.clone()];
        payload.extend(symbols);
        self.send("quote_add_symbols", payload).await?;
        Ok(())
    }

    pub async fn quote_fast_symbols(&mut self, symbols: Vec<String>) -> Result<()> {
        let mut payload = vec![self.session.clone()];
        payload.extend(symbols);
        self.send("quote_fast_symbols", payload).await?;
        Ok(())
    }

    pub async fn quote_remove_symbols(&mut self, symbols: Vec<String>) -> Result<()> {
        let mut payload = vec![self.session.clone()];
        payload.extend(symbols);
        self.send("quote_remove_symbols", payload).await?;
        Ok(())
    }

    async fn handle_message(&mut self, message: Message) -> Result<()> {
        if message.is_binary() {
            warn!("Binary message received: {:#?}", message);
            return Ok(());
        } else if message.is_text() {
            let messages = parse_packet(&message.to_string())?;
            for value in messages {
                if value.is_number() {
                    self.on_ping(&message).await?;
                } else if value["m"].is_string() && value["m"] == "qsd" {
                    let quote_data =
                        serde_json::from_value::<crate::model::Quote>(value["p"][1].clone())
                            .unwrap();
                    info!("Quote data: {:?}", quote_data);
                    return Ok(());
                }
                return Ok(());
            }
            Ok(())
        } else {
            warn!("Unknown message received: {:?}", message);
            return Ok(());
        }
    }

    pub async fn event_loop(&mut self) {
        while let Some(result) = self.read.next().await {
            match result {
                Ok(msg) => self.handle_message(msg).await.unwrap(),
                Err(e) => {
                    error!("Error reading message: {:#?}", e);
                }
            }
        }
    }

    pub async fn load(&mut self) {
        self.event_loop().await;
    }

    async fn send<M, P>(&mut self, message: M, payload: Vec<P>) -> Result<()>
    where
        M: Serialize,
        P: Serialize,
    {
        let msg = format_packet(SocketMessage::new(message, payload))?;
        self.messages.push_back(msg);
        self.send_queue().await?;
        Ok(())
    }

    async fn send_queue(&mut self) -> Result<()> {
        while !self.messages.is_empty() {
            let msg = self.messages.pop_front().unwrap();
            self.write.send(msg).await?;
        }
        Ok(())
    }

    async fn close(&mut self) -> Result<()> {
        self.write.close().await?;
        Ok(())
    }

    async fn on_connected(&mut self) {}

    async fn on_ping(&mut self, ping: &Message) -> Result<()> {
        self.write.send(ping.clone()).await?;
        Ok(())
    }

    async fn on_data(&mut self) {
        todo!()
    }

    async fn event_handler(&mut self) {
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
