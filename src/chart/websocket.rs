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

use serde::{Deserialize, Serialize};
use serde_json::Value;

use std::collections::VecDeque;

use tokio::net::TcpStream;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, protocol::Message},
    MaybeTlsStream, WebSocketStream,
};

use tracing::{debug, error, info, warn};
use url::Url;

use rayon::prelude::*;

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
struct ChartSymbolInit {
    pub adjustment: String,
    pub symbol: String,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
struct ChartDataPoint {
    #[serde(rename = "i")]
    pub id: u32,
    #[serde(rename = "v")]
    pub value: [f64; 6],
}

#[derive(Default)]
struct ChartSeries {
    id: String,
    symbol_id: String,
    symbol: String,
    interval: super::Interval,
}

pub struct ChartSocket {
    write: SplitSink<WebSocketStream<MaybeTlsStream<TcpStream>>, Message>,
    read: SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    chart_session_id: String,
    replay_session_id: String,
    messages: VecDeque<Message>,
    chart_series: Vec<ChartSeries>,
    current_series: usize,
    auth_token: String,
    replay_mode: bool,
    // handler: Box<dyn FnMut(ChartEvent, Value) -> Result<()> + 'a>,
}

pub struct ChartSocketBuilder {
    server: DataServer,
    auth_token: Option<String>,
    // handler: Option<Box<dyn FnMut(ChartEvent, Value) -> Result<()> + 'a>>,
    relay_mode: bool,
}

impl ChartSocketBuilder {
    pub fn auth_token(&mut self, auth_token: String) -> &mut Self {
        self.auth_token = Some(auth_token);
        self
    }

    pub fn relay_mode(&mut self, relay_mode: bool) -> &mut Self {
        self.relay_mode = relay_mode;
        self
    }

    fn initial_messages(&self, session: &str, auth_token: &str) -> Result<VecDeque<Message>> {
        Ok(VecDeque::from(vec![
            SocketMessage::new("set_auth_token", &[auth_token]).to_message()?,
            SocketMessage::new("chart_create_session", &[session]).to_message()?,
        ]))
    }

    pub async fn build(&mut self) -> Result<ChartSocket> {
        let url = Url::parse(&format!(
            "wss://{server}.tradingview.com/socket.io/websocket",
            server = self.server
        ))
        .unwrap();

        let mut request = url.into_client_request().unwrap();
        let headers = request.headers_mut();
        headers.insert("Origin", "https://www.tradingview.com/".parse().unwrap());
        headers.insert("User-Agent", UA.parse().unwrap());

        let socket: WebSocketStream<MaybeTlsStream<TcpStream>> = match connect_async(request).await
        {
            Ok(answer) => {
                info!("WebSocket handshake has been successfully completed");
                debug!("WebSocket handshake response: {:?}", answer.1);
                answer.0
            }
            Err(e) => {
                error!("Failed to connect: {}", e);
                return Err(Error::WebSocketError(e));
            }
        };

        let (write, read) = socket.split();

        let auth_token = match self.auth_token.clone() {
            Some(token) => token,
            None => "unauthorized_user_token".to_string(),
        };

        let chart_session_id = gen_session_id("cs");
        let replay_session_id = gen_session_id("rs");

        let messages = self.initial_messages(&chart_session_id, &auth_token)?;

        Ok(ChartSocket {
            write,
            read,
            chart_session_id,
            replay_session_id,
            messages,
            auth_token,
            chart_series: Vec::new(),
            current_series: 0,
            replay_mode: self.relay_mode,
            // handler: self.handler.take().unwrap(),
        })
    }
}

impl ChartSocket {
    pub fn new(server: DataServer) -> ChartSocketBuilder {
        ChartSocketBuilder {
            server,
            auth_token: None,
            // handler: None,
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
        self.send(
            "switch_timezone",
            &[self.chart_session_id.clone(), timezone.to_string()],
        )
        .await?;
        Ok(())
    }

    async fn handle_msg(&mut self, message: Value) -> Result<()> {
        // Define constants
        const MESSAGE_TYPE_KEY: &str = "m";
        const PAYLOAD_KEY: usize = 1;
        // Check the message type
        if let Some(message_type) = message.get(MESSAGE_TYPE_KEY).and_then(|m| m.as_str()) {
            match message_type {
                "timescale_update" => {
                    // Handle timescale update message
                    if let Some(payload) = message.get("p").and_then(|p| p.get(PAYLOAD_KEY)) {
                        // Iterate through chart series
                        for s in &self.chart_series {
                            if let Some(data) = payload
                                .get(&s.id)
                                .and_then(|s| s.get("s").and_then(|a| a.as_array()))
                            {
                                // Iterate through data points
                                data.par_iter().for_each(|v| {
                                    let data_point: ChartDataPoint =
                                        serde_json::from_value(v.clone()).unwrap();
                                    // Process the data point
                                    debug!("data point: {:#?}", data_point);
                                });
                            }
                        }
                    }
                }
                "du" => {}

                "series_loading" => {}
                "series_completed" => {}

                "symbol_resolved" => {}

                "critical_error" => {}
                "series_error" => {}
                "symbol_error" => {}

                "replay_ok" => {}
                "replay_instance_id" => {}
                "replay_point" => {}
                "replay_resolutions" => {}
                "replay_data_end" => {}
                _ => {
                    debug!("unhandled message: {:#?}", message)
                }
            }
        }
        Ok(())
    }

    pub async fn event_loop(&mut self) {
        self.create_series("BINANCE:BTCUSDT", super::Interval::FiveMinutes, 10)
            .await
            .unwrap();

        while let Some(result) = self.read.next().await {
            match result {
                Ok(message) => {
                    let values = parse_packet(&message.to_string()).unwrap();
                    for value in values {
                        match value {
                            Value::Number(_) => match self.ping(&message).await {
                                Ok(_) => debug!("ping sent"),
                                Err(e) => {
                                    warn!("ping failed with: {:#?}", e);
                                }
                            },
                            Value::Object(_) => match self.handle_msg(value).await {
                                Ok(()) => {}
                                Err(e) => {
                                    error!("unable to handle message, with: {:#?}", e);
                                }
                            },
                            _ => (),
                        }
                    }
                }
                Err(e) => {
                    error!("Error reading message: {:#?}", e);
                }
            }
        }
    }

    pub async fn create_series(
        &mut self,
        symbol: &str,
        interval: super::Interval,
        dp_num: u64,
    ) -> Result<()> {
        let series_id = format!("sds_{}", self.current_series);
        let series_symbol_id = format!("sds_sym_{}", self.current_series);
        self.current_series += 1;
        self.chart_series.push(ChartSeries {
            id: series_id.clone(),
            symbol_id: series_symbol_id.clone(),
            symbol: symbol.to_string(),
            interval: interval.clone(),
        });
        let symbol_init = ChartSymbolInit {
            adjustment: "splits".to_string(),
            symbol: symbol.to_string(),
        };
        let symbol_init_json = serde_json::to_value(&symbol_init)?;
        let resolve_args = &[
            self.chart_session_id.clone(),
            series_symbol_id.clone(),
            format!("={}", symbol_init_json),
        ];
        self.send("resolve_symbol", resolve_args).await?;
        let create_series_args = &[
            Value::from(self.chart_session_id.clone()),
            Value::from(series_id),
            Value::from("s1"),
            Value::from(series_symbol_id.clone()),
            Value::from(interval.to_string()),
            Value::from(dp_num),
        ];
        self.send("create_series", create_series_args).await?;
        Ok(())
    }

    async fn _delete_chart_session_id(&mut self) -> Result<()> {
        self.send("chart_delete_session", &[self.chart_session_id.clone()])
            .await?;
        Ok(())
    }

    async fn _delete_replay_session_id(&mut self) -> Result<()> {
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
