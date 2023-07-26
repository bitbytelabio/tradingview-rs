use crate::{
    error::SocketError,
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
}

impl QuoteSocketBuilder {
    pub fn auth_token(mut self, auth_token: String) -> Self {
        self.auth_token = Some(auth_token);
        self
    }

    fn set_quote_fields(session: &str) -> Vec<String> {
        let mut params = vec![session.to_string()];
        super::ALL_QUOTE_FIELDS.iter().for_each(|field| {
            params.push(field.to_string());
        });
        params
    }

    fn set_connection_setup_messages(
        auth_token: &str,
        session: &str,
    ) -> Result<VecDeque<Message>, Box<dyn std::error::Error>> {
        let messages = vec![
            SocketMessage::new("set_auth_token", vec![auth_token]).to_message()?,
            SocketMessage::new("quote_create_session", vec![session]).to_message()?,
            SocketMessage::new("quote_set_fields", Self::set_quote_fields(session)).to_message()?,
        ];

        Ok(VecDeque::from(messages))
    }

    pub async fn build(&mut self) -> Result<QuoteSocket, Box<dyn std::error::Error>> {
        let url = Url::parse(&format!(
            "wss://{server}.tradingview.com/socket.io/websocket",
            server = self.server.to_string()
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

        let auth_token = match self.auth_token.clone() {
            Some(token) => token,
            None => "unauthorized_user_token".to_string(),
        };

        let session = gen_session_id("qs");

        let messages = Self::set_connection_setup_messages(&auth_token, &session)?;

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
        };
    }

    async fn handle_message(message: Message) {
        if message.is_binary() {
            warn!("Binary message received: {:#?}", message);
        } else if message.is_text() {
            warn!("Text message received: {:#?}", message);
        } else {
            warn!("Unknown message received: {:#?}", message);
        }
    }

    async fn event_loop(&mut self) {
        while let Some(result) = self.read.next().await {
            match result {
                Ok(msg) => {
                    let parsed_msg = parse_packet(&msg.to_string()).unwrap();
                    for x in parsed_msg {
                        if x.is_number() {
                            let y = self.write.send(msg.clone()).await;
                            match y {
                                Ok(_) => {
                                    debug!("Message sent successfully: {:#?}", msg.to_string())
                                }
                                Err(e) => error!("Error sending message: {:#?}", e),
                            }
                        } else if x["m"].is_string() && x["m"] == "qsd" {
                            let quote_data =
                                serde_json::from_value::<crate::model::Quote>(x["p"][1].clone())
                                    .unwrap();
                            info!("Quote data: {:#?}", quote_data);
                        }
                        debug!("Message received: {:#?}", x);
                    }
                }
                Err(e) => {
                    error!("Error reading message: {:#?}", e);
                }
            }
        }
    }

    pub async fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.send(
            "quote_add_symbols",
            vec![self.session.clone(), "BINANCE:BTCUSDT".to_owned()],
        )
        .await?;

        while let Some(result) = self.read.next().await {
            match result {
                Ok(msg) => {
                    warn!("is ping?: {:?}", msg.is_text());
                    let parsed_msg = parse_packet(&msg.to_string()).unwrap();
                    for x in parsed_msg {
                        if x.is_number() {
                            let y = self.write.send(msg.clone()).await;
                            match y {
                                Ok(_) => {
                                    debug!("Message sent successfully: {:#?}", msg.to_string())
                                }
                                Err(e) => error!("Error sending message: {:#?}", e),
                            }
                        } else if x["m"].is_string() && x["m"] == "qsd" {
                            let quote_data =
                                serde_json::from_value::<crate::model::Quote>(x["p"][1].clone())
                                    .unwrap();
                            // info!("Quote data: {:#?}", quote_data);
                        }
                        // debug!("Message received: {:#?}", x);
                    }
                }
                Err(e) => {
                    error!("Error reading message: {:#?}", e);
                }
            }
        }
        Ok(())
    }

    async fn send<M, P>(
        &mut self,
        message: M,
        payload: Vec<P>,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        M: Serialize,
        P: Serialize,
    {
        let msg = match format_packet(SocketMessage::new(message, payload)) {
            Ok(msg) => msg,
            Err(e) => return Err(e),
        };

        self.messages.push_back(msg);
        self.send_queue().await?;
        Ok(())
    }

    async fn send_queue(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        while !self.messages.is_empty() {
            let msg = self.messages.pop_front().unwrap();
            self.write.send(msg).await?;
        }
        Ok(())
    }

    async fn close(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.write.close().await?;
        Ok(())
    }

    async fn on_connected(&mut self) {}

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
