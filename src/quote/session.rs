use crate::{
    payload,
    prelude::*,
    quote::{
        QuotePayload, QuotePayloadType, QuoteSocketEvent, QuoteSocketMessage, ALL_QUOTE_FIELDS,
    },
    socket::{DataServer, Socket, SocketMessage, SocketMessageType},
    utils::{gen_session_id, parse_packet},
};
use async_trait::async_trait;
use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use rayon::prelude::*;
use serde::Serialize;
use serde_json::Value;
use std::{borrow::Cow, collections::VecDeque, rc::Rc, sync::Arc, thread::JoinHandle};
use tokio::time::{timeout, Duration};
use tokio::{net::TcpStream, sync::Mutex};
use tokio_tungstenite::{tungstenite::protocol::Message, MaybeTlsStream, WebSocketStream};
use tracing::{debug, error, info, warn};

// impl<'a> QuoteSocketBuilder<'a> {
//     pub fn auth_token(&mut self, auth_token: String) -> &mut Self {
//         self.auth_token = Some(auth_token);
//         self
//     }

//     pub fn quote_fields(&mut self, quote_fields: Vec<String>) -> &mut Self {
//         self.quote_fields = Some(quote_fields);
//         self
//     }

//     fn initial_messages(&self, session: &str, auth_token: &str) -> Result<VecDeque<Message>> {
//         let quote_fields: Vec<Value> = match self.quote_fields.clone() {
//             Some(fields) => {
//                 let mut quote_fields = payload![session.clone().to_string()];
//                 quote_fields.extend(fields.into_iter().map(Value::from));
//                 quote_fields
//             }
//             None => {
//                 let mut quote_fields = payload![session.clone().to_string()];
//                 quote_fields.extend(ALL_QUOTE_FIELDS.clone().into_iter().map(Value::from));
//                 quote_fields
//             }
//         };

//         Ok(VecDeque::from(vec![
//             SocketMessage::new("set_auth_token", payload!(auth_token)).to_message()?,
//             SocketMessage::new("quote_create_session", payload!(session)).to_message()?,
//             SocketMessage::new("quote_set_fields", quote_fields).to_message()?,
//         ]))
//     }

//     pub async fn build(&mut self) -> Result<QuoteSocket> {
//         let url = Url::parse(&format!(
//             "wss://{server}.tradingview.com/socket.io/websocket",
//             server = self.server
//         ))
//         .unwrap();

//         let mut request = url.into_client_request().unwrap();
//         request.headers_mut().extend(WEBSOCKET_HEADERS.clone());

//         let socket: WebSocketStream<MaybeTlsStream<TcpStream>> = match connect_async(request).await
//         {
//             Ok(answer) => {
//                 info!("WebSocket handshake has been successfully completed");
//                 debug!("WebSocket handshake response: {:?}", answer.1);
//                 answer.0
//             }
//             Err(e) => {
//                 error!("Failed to connect: {}", e);
//                 return Err(Error::WebSocketError(e));
//             }
//         };

//         let (write, read) = socket.split();

//         let auth_token = match self.auth_token.clone() {
//             Some(token) => token,
//             None => "unauthorized_user_token".to_string(),
//         };

//         let quote_session_id = gen_session_id("qs");

//         let messages = self.initial_messages(&quote_session_id, &auth_token)?;

//         Ok(QuoteSocket {
//             write,
//             read,
//             quote_session_id,
//             messages,
//             auth_token,
//             handler: self.handler.take().unwrap(),
//         })
//     }
// }

// impl<'a> QuoteSocket<'a> {
//     pub fn new<CallbackFn>(server: DataServer, handler: CallbackFn) -> QuoteSocketBuilder<'a>
//     where
//         CallbackFn: FnMut(QuoteEvent, Value) -> Result<()> + 'a,
//     {
//         QuoteSocketBuilder {
//             server,
//             auth_token: None,
//             quote_fields: None,
//             handler: Some(Box::new(handler)),
//         }
//     }

//     pub async fn quote_add_symbols(&mut self, symbols: Vec<String>) -> Result<()> {
//         let mut payloads = payload![self.quote_session_id.clone()];
//         payloads.extend(symbols.into_iter().map(Value::from));
//         self.send("quote_add_symbols", payloads).await?;
//         Ok(())
//     }

//     pub async fn quote_fast_symbols(&mut self, symbols: Vec<String>) -> Result<()> {
//         let mut payloads = payload![self.quote_session_id.clone()];
//         payloads.extend(symbols.into_iter().map(Value::from));
//         self.send("quote_fast_symbols", payloads).await?;
//         Ok(())
//     }

//     pub async fn quote_remove_symbols(&mut self, symbols: Vec<String>) -> Result<()> {
//         let mut payloads = payload![self.quote_session_id.clone()];
//         payloads.extend(symbols.into_iter().map(Value::from));
//         self.send("quote_remove_symbols", payloads).await?;
//         Ok(())
//     }

//     async fn handle_message(&mut self, message: Value) -> Result<()> {
//         let payload: SocketMessageType<QuoteSocketMessage> = serde_json::from_value(message)
//             .map_err(|e| {
//                 error!("Error parsing quote data: {:#?}", e);
//                 Error::JsonParseError(e)
//             })?;

//         match payload {
//             SocketMessageType::SocketServerInfo(server_info) => {
//                 info!("Received SocketServerInfo: {:#?}", server_info);
//             }
//             SocketMessageType::SocketMessage(quote_msg) => match quote_msg.message_type.as_str() {
//                 "qsd" => {
//                     quote_msg.payload.par_iter().for_each(|p| match p {
//                         QuotePayloadType::String(s) => debug!("Received quote session ID: {}", s),
//                         QuotePayloadType::QuotePayload(d) => {
//                             info!("Received quote data: {:#?}", d);
//                         }
//                     });
//                 }
//                 "quote_completed" => {
//                     info!("Received quote completed message: {:#?}", quote_msg);
//                 }
//                 _ => {}
//             },
//         }
//         Ok(())
//     }

//     pub async fn event_loop(&mut self) {
//         loop {
//             match timeout(Duration::from_secs(5), self.read.next()).await {
//                 Ok(Some(result)) => match result {
//                     Ok(message) => {
//                         if let Message::Text(text) = &message {
//                             if let Ok(values) = parse_packet(text) {
//                                 for value in values {
//                                     match value {
//                                         Value::Number(_) => {
//                                             if let Err(e) = self.ping(&message).await {
//                                                 error!("Error handling ping: {:#?}", e);
//                                             }
//                                         }
//                                         Value::Object(_) => {
//                                             if let Err(e) = self.handle_message(value).await {
//                                                 error!("Error handling message: {:#?}", e);
//                                             }
//                                         }
//                                         _ => (),
//                                     }
//                                 }
//                             } else if let Err(e) = parse_packet(text) {
//                                 error!("Error parsing message: {:#?}", e);
//                             }
//                         } else {
//                             warn!("Received non-text message: {:?}", message);
//                         }
//                     }
//                     Err(e) => {
//                         error!("Error reading message: {:#?}", e);
//                         break;
//                     }
//                 },
//                 Ok(None) => {
//                     // Connection closed
//                     break;
//                 }
//                 Err(e) => {
//                     error!("Error reading message: {:#?}", e);
//                     break;
//                 }
//             }
//         }
//     }

//     async fn send<M, P>(&mut self, message: M, payload: Vec<P>) -> Result<()>
//     where
//         M: Serialize,
//         P: Serialize,
//     {
//         let msg = format_packet(SocketMessage::new(message, payload))?;
//         self.messages.push_back(msg);
//         self.send_queue().await?;
//         Ok(())
//     }

//     async fn send_queue(&mut self) -> Result<()> {
//         while let Some(msg) = self.messages.pop_front() {
//             self.write.send(msg).await?;
//         }
//         Ok(())
//     }

//     async fn ping(&mut self, ping: &Message) -> Result<()> {
//         self.write.send(ping.clone()).await?;
//         Ok(())
//     }

//     pub async fn delete_session(&mut self) -> Result<()> {
//         self.send(
//             "quote_delete_session",
//             payload!(self.quote_session_id.clone()),
//         )
//         .await?;
//         Ok(())
//     }

//     pub async fn update_session(&mut self) -> Result<()> {
//         self.delete_session().await?;
//         self.quote_session_id = gen_session_id("qs");
//         self.send(
//             "quote_create_session",
//             payload!(self.quote_session_id.clone()),
//         )
//         .await?;
//         Ok(())
//     }

//     pub async fn quote_set_fields(&mut self, fields: Vec<String>) -> Result<()> {
//         let mut payload = vec![self.quote_session_id.clone()];
//         payload.extend(fields);
//         self.send("quote_set_fields", payload!(payload)).await?;
//         Ok(())
//     }

//     pub async fn set_auth_token(&mut self, auth_token: &str) -> Result<()> {
//         self.auth_token = auth_token.to_string();
//         self.send("set_auth_token", payload!(self.auth_token.clone()))
//             .await?;
//         Ok(())
//     }
// }

pub struct WebSocketsBuilder {
    server: Option<DataServer>,
    auth_token: Option<String>,
    quote_fields: Option<Vec<String>>,
}

pub struct WebSocket {
    write: Arc<Mutex<SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>>>,
    read: Arc<Mutex<SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>>>,
    quote_session_id: String,
    quote_fields: Vec<Value>,
    auth_token: String,
    callback_fn: Box<dyn FnMut(QuoteSocketEvent, QuotePayload) -> Result<()> + Send + Sync>,
}

impl WebSocketsBuilder {
    pub fn new() -> Self {
        Self {
            auth_token: None,
            server: None,
            quote_fields: None,
        }
    }

    pub fn server(&mut self, server: DataServer) -> &mut Self {
        self.server = Some(server);
        self
    }

    pub fn auth_token(&mut self, auth_token: String) -> &mut Self {
        self.auth_token = Some(auth_token);
        self
    }

    pub fn quote_fields(&mut self, quote_fields: Vec<String>) -> &mut Self {
        self.quote_fields = Some(quote_fields);
        self
    }

    pub async fn connect<T>(&self, callback: T) -> Result<WebSocket>
    where
        T: FnMut(QuoteSocketEvent, QuotePayload) -> Result<()> + Send + Sync + 'static,
    {
        let auth_token = self
            .auth_token
            .clone()
            .unwrap_or("unauthorized_user_token".to_string());

        let server = self.server.clone().unwrap_or_default();

        let session = gen_session_id("qs");

        let quote_fields: Vec<Value> = match self.quote_fields.clone() {
            Some(fields) => {
                let mut quote_fields = payload![session.clone().to_string()];
                quote_fields.extend(fields.into_iter().map(Value::from));
                quote_fields
            }
            None => {
                let mut quote_fields = payload![session.clone().to_string()];
                quote_fields.extend(ALL_QUOTE_FIELDS.clone().into_iter().map(Value::from));
                quote_fields
            }
        };

        let (write_stream, read_stream) = WebSocket::connect(&server, &auth_token).await?;

        let write: Arc<Mutex<SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>>> =
            Arc::from(Mutex::new(write_stream));

        let read = Arc::from(Mutex::new(read_stream));

        Ok(WebSocket {
            write,
            read,
            quote_session_id: session,
            quote_fields,
            auth_token,
            callback_fn: Box::new(callback),
        })
    }
}

impl WebSocket {
    pub fn new() -> WebSocketsBuilder {
        WebSocketsBuilder::new()
    }

    pub async fn create_session(&mut self) -> Result<()> {
        self.send(
            "quote_create_session",
            &payload!(self.quote_session_id.clone()),
        )
        .await?;
        Ok(())
    }

    pub async fn delete_session(&mut self) -> Result<()> {
        self.send(
            "quote_delete_session",
            &payload!(self.quote_session_id.clone()),
        )
        .await?;
        Ok(())
    }

    pub async fn update_session(&mut self) -> Result<()> {
        self.delete_session().await?;
        self.quote_session_id = gen_session_id("qs");
        self.send(
            "quote_create_session",
            &payload!(self.quote_session_id.clone()),
        )
        .await?;
        Ok(())
    }

    pub async fn set_fields(&mut self) -> Result<()> {
        self.send("quote_set_fields", &self.quote_fields.clone())
            .await?;
        Ok(())
    }

    pub async fn add_symbols(&mut self, symbols: Vec<&str>) -> Result<()> {
        let mut payloads = payload![self.quote_session_id.clone()];
        payloads.extend(symbols.into_iter().map(Value::from));
        self.send("quote_add_symbols", &payloads).await?;
        Ok(())
    }

    pub async fn update_auth_token(&mut self, auth_token: &str) -> Result<()> {
        self.auth_token = auth_token.to_owned();
        self.send("set_auth_token", &payload!(auth_token)).await?;
        Ok(())
    }

    pub async fn fast_symbols(&mut self, symbols: Vec<&str>) -> Result<()> {
        let mut payloads = payload![self.quote_session_id.clone()];
        payloads.extend(symbols.into_iter().map(Value::from));
        self.send("quote_fast_symbols", &payloads).await?;
        Ok(())
    }

    pub async fn remove_symbols(&mut self, symbols: Vec<&str>) -> Result<()> {
        let mut payloads = payload![self.quote_session_id.clone()];
        payloads.extend(symbols.into_iter().map(Value::from));
        self.send("quote_remove_symbols", &payloads).await?;
        Ok(())
    }

    pub async fn subscribe(&mut self) {
        self.event_loop().await;
    }
}

#[async_trait]
impl Socket for WebSocket {
    async fn send(&mut self, m: &str, p: &[Value]) -> Result<()> {
        self.write
            .lock()
            .await
            .send(SocketMessage::new(m, p).to_message()?)
            .await?;
        Ok(())
    }

    async fn ping(&mut self, ping: &Message) -> Result<()> {
        self.write.lock().await.send(ping.clone()).await?;
        Ok(())
    }
    async fn event_loop(&mut self) {
        loop {
            let read = self.read.clone();
            let mut read_guard = read.lock().await;
            match timeout(Duration::from_secs(5), read_guard.next()).await {
                Ok(Some(result)) => match result {
                    Ok(message) => {
                        if let Message::Text(text) = &message {
                            if let Ok(values) = parse_packet(text) {
                                for value in values {
                                    match value {
                                        Value::Number(_) => {
                                            if let Err(e) = self.ping(&message).await {
                                                error!("Error handling ping: {:#?}", e);
                                            }
                                        }
                                        Value::Object(_) => {
                                            if let Err(e) = self.handle_message(value).await {
                                                error!("Error handling message: {:#?}", e);
                                            }
                                        }
                                        _ => (),
                                    }
                                }
                            } else if let Err(e) = parse_packet(text) {
                                error!("Error parsing message: {:#?}", e);
                            }
                        } else {
                            warn!("Received non-text message: {:?}", message);
                        }
                    }
                    Err(e) => {
                        error!("Error reading message: {:#?}", e);
                        break;
                    }
                },
                Ok(None) => {
                    // Connection closed
                    break;
                }
                Err(e) => {
                    error!("Error reading message: {:#?}", e);
                    break;
                }
            }
        }
    }

    async fn handle_message(&mut self, message: Value) -> Result<()> {
        let payload: SocketMessageType<QuoteSocketMessage> = serde_json::from_value(message)
            .map_err(|e| {
                error!("error parsing quote data: {:#?}", e);
                Error::JsonParseError(e)
            })?;
        match payload {
            SocketMessageType::SocketServerInfo(server_info) => {
                info!("Received SocketServerInfo: {:#?}", server_info);
            }
            SocketMessageType::SocketMessage(quote_msg) => match quote_msg.message_type.as_str() {
                "qsd" => {
                    if let Some(payload) = quote_msg.payload.get(1) {
                        if let QuotePayloadType::QuotePayload(quote_payload) = payload {
                            // info!("Received quote message: {:#?}", payload);
                            (self.callback_fn)(QuoteSocketEvent::Data, quote_payload.clone())
                                .unwrap();
                        }
                    }
                }
                "quote_completed" => {
                    // info!("Received quote completed message: {:#?}", quote_msg);
                }
                _ => {}
            },
        }
        Ok(())
    }

    async fn handle_error(&mut self, message: Value) -> Result<()> {
        Ok(())
    }
    async fn handle_data(&mut self, payload: Value) -> Result<()> {
        Ok(())
    }
    async fn handle_loaded(&mut self, payload: Value) -> Result<()> {
        Ok(())
    }
}
