use crate::{
    chart::ChartEvent,
    prelude::*,
    socket::{DataServer, SocketMessage},
    utils::{format_packet, gen_session_id, parse_packet},
    UA,
};

use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
use serde::Serialize;
use serde_json::Value as JsonValue;

use std::borrow::Cow;
use std::collections::VecDeque;

use tokio::net::TcpStream;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, protocol::Message},
    MaybeTlsStream, WebSocketStream,
};

use tracing::{error, info};
use url::Url;

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
}

impl<'a> ChartSocketBuilder<'a> {
    pub fn auth_token(&mut self, auth_token: String) -> &mut Self {
        self.auth_token = Some(auth_token);
        self
    }
}

impl<'a> ChartSocket<'a> {}
