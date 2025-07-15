use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;
use ustr::Ustr;

use crate::{
    QuoteValue,
    live::handler::{EventHandler, command::Command},
    websocket::WebSocketClient,
};

pub struct QuoteStreamClient {
    ws: Option<WebSocketClient>,
    event_handler: Arc<EventHandler>,
    quotes: Arc<DashMap<Ustr, QuoteValue>>,

    command_tx: Option<UnboundedSender<Command>>,
}

impl QuoteStreamClient {
    pub fn new(auth_token: Option<&str>) -> Arc<Self> {
        todo!()
    }
}
