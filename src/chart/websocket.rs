use crate::{
    chart::ChartEvent,
    prelude::*,
    socket::{DataServer, SocketMessage},
    utils::{format_packet, gen_session_id, parse_packet},
    UA,
};

use futures_util::stream::{SplitSink, SplitStream};

use serde_json::Value as JsonValue;

use std::collections::VecDeque;

use tokio::net::TcpStream;
use tokio_tungstenite::{tungstenite::protocol::Message, MaybeTlsStream, WebSocketStream};

pub struct ChartSocket<'a> {
    write: SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>,
    read: SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    session: String,
    relay_session: String,
    messages: VecDeque<Message>,
    auth_token: String,
    handler: Box<dyn FnMut(ChartEvent, JsonValue) -> Result<()> + 'a>,
}

pub struct ChartSocketBuilder<'a> {
    server: DataServer,
    auth_token: Option<String>,
    quote_fields: Option<Vec<String>>,
    handler: Option<Box<dyn FnMut(ChartEvent, JsonValue) -> Result<()> + 'a>>,
    relay_mode: bool,
}

impl<'a> ChartSocketBuilder<'a> {
    pub fn auth_token(&mut self, auth_token: String) -> &mut Self {
        self.auth_token = Some(auth_token);
        self
    }

    pub async fn build(&mut self) -> ChartSocket {
        let session = gen_session_id("cs");
        let relay_session = gen_session_id("rs");
        todo!()
    }
}

impl<'a> ChartSocket<'a> {
    pub fn new(server: DataServer) -> ChartSocketBuilder<'a> {
        ChartSocketBuilder {
            server,
            auth_token: None,
            quote_fields: None,
            handler: None,
            relay_mode: false,
        }
    }
}
