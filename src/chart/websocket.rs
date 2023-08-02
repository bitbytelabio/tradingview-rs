use crate::{
    chart::ChartEvent,
    prelude::*,
    socket::{DataServer, SocketMessage},
    utils::{format_packet, gen_session_id},
};

use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt,
};

use serde::Serialize;
use serde_json::Value as JsonValue;

use std::collections::VecDeque;

use tokio::net::TcpStream;
use tokio_tungstenite::{tungstenite::protocol::Message, MaybeTlsStream, WebSocketStream};

pub struct ChartSocket<'a> {
    write: SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>,
    read: SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    chart_session_id: String,
    replay_session_id: String,
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
        let _session = gen_session_id("cs");
        let _replay_session = gen_session_id("rs");
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

    pub async fn set_local(&mut self, local: &[String]) -> Result<()> {
        self.send("set_local", local).await?;
        Ok(())
    }

    pub async fn set_auth_token(&mut self, auth_token: &str) -> Result<()> {
        self.auth_token = auth_token.to_string();
        self.send("set_auth_token", &[self.auth_token.clone()])
            .await?;
        Ok(())
    }

    pub async fn set_timezone(&mut self, timezone: &str) -> Result<()> {
        self.send("switch_timezone", &[timezone]).await?;
        Ok(())
    }

    async fn delete_chart_session_id(&mut self) -> Result<()> {
        self.send("chart_delete_session", &[self.chart_session_id.clone()])
            .await?;
        Ok(())
    }

    async fn delete_replay_session_id(&mut self) -> Result<()> {
        self.send("replay_delete_session", &[self.chart_session_id.clone()])
            .await?;
        Ok(())
    }

    async fn send<M, P>(&mut self, message: M, payload: &[P]) -> Result<()>
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

    async fn ping(&mut self, ping: &Message) -> Result<()> {
        self.write.send(ping.clone()).await?;
        Ok(())
    }

    pub async fn fetch_more_data(&mut self, num: u64) -> Result<()> {
        self.send(
            "request_more_data",
            &[
                self.chart_session_id.clone(),
                "$prices".to_string(),
                num.to_string(),
            ],
        )
        .await?;
        Ok(())
    }
}
