use crate::{
    DataPoint, Error, Interval, Result, Timezone,
    callback::Callbacks,
    chart::{
        ChartOptions, StudyOptions,
        models::{ChartResponseData, StudyResponseData, SymbolInfo},
    },
    error::TradingViewError,
    payload,
    pine_indicator::PineIndicator,
    quote::{
        ALL_QUOTE_FIELDS,
        models::{QuoteData, QuoteValue},
        utils::merge_quotes,
    },
    socket::{DataServer, Socket, SocketMessageDe, SocketSession, TradingViewDataEvent},
    utils::{gen_id, gen_session_id, symbol_init},
};
use serde::Deserialize;
use serde_json::Value;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;
use tracing::{debug, error, trace, warn};

#[derive(Clone, Default)]
pub struct WebSocketClient {
    metadata: Metadata,
    callbacks: Callbacks,
}

#[derive(Default, Clone)]
struct Metadata {
    series_count: u16,
    series: HashMap<String, SeriesInfo>,
    studies_count: u16,
    studies: HashMap<String, String>,
    quotes: HashMap<String, QuoteValue>,
    quote_session: String,
    chart_data_cache: Arc<RwLock<Option<Vec<DataPoint>>>>,
}

#[derive(Clone)]
pub struct WebSocket {
    client: Arc<RwLock<WebSocketClient>>,
    socket: SocketSession,
}

#[derive(Default)]
pub struct WebSocketBuilder {
    client: Option<WebSocketClient>,
    auth_token: Option<String>,
    server: Option<DataServer>,
}

#[derive(Debug, Clone, Default)]
pub struct SeriesInfo {
    pub chart_session: String,
    pub options: ChartOptions,
}

impl WebSocketBuilder {
    pub fn client(mut self, client: WebSocketClient) -> Self {
        self.client = Some(client);
        self
    }

    pub fn auth_token(mut self, auth_token: &str) -> Self {
        self.auth_token = Some(auth_token.to_string());
        self
    }

    pub fn server(mut self, server: DataServer) -> Self {
        self.server = Some(server);
        self
    }

    pub async fn build(self) -> Result<WebSocket> {
        let auth_token = self
            .auth_token
            .unwrap_or("unauthorized_user_token".to_string());
        let server = self.server.unwrap_or_default();

        let socket = SocketSession::new(server, auth_token).await?;
        let client = self.client.unwrap_or_default();

        Ok(WebSocket::new_with_session(client, socket))
    }
}

impl WebSocket {
    #[allow(clippy::new_ret_no_self)]
    pub fn new() -> WebSocketBuilder {
        WebSocketBuilder::default()
    }

    pub fn new_with_session(client: WebSocketClient, socket: SocketSession) -> Self {
        Self {
            client: Arc::new(RwLock::new(client)),
            socket,
        }
    }

    // Begin TradingView WebSocket Quote methods
    pub async fn create_quote_session(&self) -> Result<&Self> {
        let quote_session = gen_session_id("qs");
        self.client.write().await.metadata.quote_session = quote_session.clone();
        self.socket
            .send("quote_create_session", &payload!(quote_session))
            .await?;
        Ok(self)
    }

    pub async fn delete_quote_session(&self) -> Result<&Self> {
        self.socket
            .send(
                "quote_delete_session",
                &payload!(self.client.read().await.metadata.quote_session.clone()),
            )
            .await?;
        Ok(self)
    }

    pub async fn set_fields(&self) -> Result<&Self> {
        let mut quote_fields = payload![
            self.client
                .read()
                .await
                .metadata
                .quote_session
                .clone()
                .to_string()
        ];
        quote_fields.extend(ALL_QUOTE_FIELDS.clone().into_iter().map(Value::from));
        self.socket.send("quote_set_fields", &quote_fields).await?;
        Ok(self)
    }

    pub async fn add_symbols(&self, symbols: Vec<&str>) -> Result<&Self> {
        let mut payloads = payload![self.client.read().await.metadata.quote_session.clone()];
        payloads.extend(symbols.into_iter().map(Value::from));
        self.socket.send("quote_add_symbols", &payloads).await?;
        Ok(self)
    }

    pub async fn update_auth_token(&mut self, auth_token: &str) -> Result<&mut Self> {
        self.socket.update_auth_token(auth_token).await?;
        Ok(self)
    }

    pub async fn fast_symbols(&self, symbols: Vec<&str>) -> Result<&Self> {
        let mut payloads = payload![self.client.read().await.metadata.quote_session.clone()];
        payloads.extend(symbols.into_iter().map(Value::from));
        self.socket.send("quote_fast_symbols", &payloads).await?;
        Ok(self)
    }

    pub async fn remove_symbols(&self, symbols: Vec<&str>) -> Result<&Self> {
        let mut payloads = payload![self.client.read().await.metadata.quote_session.clone()];
        payloads.extend(symbols.into_iter().map(Value::from));
        self.socket.send("quote_remove_symbols", &payloads).await?;
        Ok(self)
    }
    // End TradingView WebSocket Quote methods

    // Begin TradingView WebSocket Chart methods
    /// Example: local = ("en", "US")
    pub async fn set_locale(&self, local: (&str, &str)) -> Result<&Self> {
        self.socket
            .send("set_locale", &payload!(local.0, local.1))
            .await?;
        Ok(self)
    }

    pub async fn set_data_quality(&self, data_quality: &str) -> Result<&Self> {
        self.socket
            .send("set_data_quality", &payload!(data_quality))
            .await?;

        Ok(self)
    }

    pub async fn set_timezone(&self, session: &str, timezone: Timezone) -> Result<&Self> {
        self.socket
            .send("switch_timezone", &payload!(session, timezone.to_string()))
            .await?;

        Ok(self)
    }

    pub async fn create_chart_session(&self, session: &str) -> Result<&Self> {
        self.socket
            .send("chart_create_session", &payload!(session))
            .await?;
        Ok(self)
    }

    pub async fn create_replay_session(&self, session: &str) -> Result<&Self> {
        self.socket
            .send("replay_create_session", &payload!(session))
            .await?;
        Ok(self)
    }

    pub async fn add_replay_series(
        &self,
        session: &str,
        series_id: &str,
        symbol: &str,
        config: &ChartOptions,
    ) -> Result<&Self> {
        self.socket
            .send(
                "replay_add_series",
                &payload!(
                    session,
                    series_id,
                    symbol_init(
                        symbol,
                        config.adjustment.clone(),
                        config.currency,
                        config.session_type.clone(),
                        None
                    )?,
                    config.interval.to_string()
                ),
            )
            .await?;
        Ok(self)
    }

    pub async fn delete_chart_session_id(&self, session: &str) -> Result<&Self> {
        self.socket
            .send("chart_delete_session", &payload!(session))
            .await?;
        Ok(self)
    }

    pub async fn delete_replay_session_id(&self, session: &str) -> Result<&Self> {
        self.socket
            .send("replay_delete_session", &payload!(session))
            .await?;
        Ok(self)
    }

    pub async fn replay_step(&self, session: &str, series_id: &str, step: u64) -> Result<&Self> {
        self.socket
            .send("replay_step", &payload!(session, series_id, step))
            .await?;
        Ok(self)
    }

    pub async fn replay_start(
        &self,
        session: &str,
        series_id: &str,
        interval: Interval,
    ) -> Result<&Self> {
        self.socket
            .send(
                "replay_start",
                &payload!(session, series_id, interval.to_string()),
            )
            .await?;
        Ok(self)
    }

    pub async fn replay_stop(&self, session: &str, series_id: &str) -> Result<&Self> {
        self.socket
            .send("replay_stop", &payload!(session, series_id))
            .await?;
        Ok(self)
    }

    pub async fn replay_reset(
        &self,
        session: &str,
        series_id: &str,
        timestamp: i64,
    ) -> Result<&Self> {
        self.socket
            .send("replay_reset", &payload!(session, series_id, timestamp))
            .await?;
        Ok(self)
    }

    pub async fn request_more_data(
        &self,
        session: &str,
        series_id: &str,
        num: u64,
    ) -> Result<&Self> {
        self.socket
            .send("request_more_data", &payload!(session, series_id, num))
            .await?;
        Ok(self)
    }

    pub async fn request_more_tickmarks(
        &self,
        session: &str,
        series_id: &str,
        num: u64,
    ) -> Result<&Self> {
        self.socket
            .send("request_more_tickmarks", &payload!(session, series_id, num))
            .await?;
        Ok(self)
    }

    pub async fn create_study(
        &self,
        session: &str,
        study_id: &str,
        series_id: &str,
        indicator: PineIndicator,
    ) -> Result<&Self> {
        let inputs = indicator.to_study_inputs()?;
        let payloads: Vec<Value> = vec![
            Value::from(session),
            Value::from(study_id),
            Value::from("st1"),
            Value::from(series_id),
            Value::from(indicator.script_type.to_string()),
            inputs,
        ];
        self.socket.send("create_study", &payloads).await?;
        Ok(self)
    }

    pub async fn modify_study(
        &self,
        session: &str,
        study_id: &str,
        series_id: &str,
        indicator: PineIndicator,
    ) -> Result<&Self> {
        let inputs = indicator.to_study_inputs()?;
        let payloads: Vec<Value> = vec![
            Value::from(session),
            Value::from(study_id),
            Value::from("st1"),
            Value::from(series_id),
            Value::from(indicator.script_type.to_string()),
            inputs,
        ];
        self.socket.send("modify_study", &payloads).await?;
        Ok(self)
    }

    pub async fn remove_study(&self, session: &str, study_id: &str) -> Result<&Self> {
        self.socket
            .send("remove_study", &payload!(session, study_id))
            .await?;
        Ok(self)
    }

    pub async fn create_series(
        &self,
        session: &str,
        series_id: &str,
        series_version: &str,
        series_symbol_id: &str,
        config: &ChartOptions,
    ) -> Result<&Self> {
        let range = match (&config.range, config.from, config.to) {
            (Some(range), _, _) => range.clone(),
            (None, Some(from), Some(to)) => format!("r,{}:{}", from, to),
            _ => String::default(),
        };
        self.socket
            .send(
                "create_series",
                &payload!(
                    session,
                    series_id,
                    series_version,
                    series_symbol_id,
                    config.interval.to_string(),
                    config.bar_count,
                    range // |r,1626220800:1628640000|1D|5d|1M|3M|6M|YTD|12M|60M|ALL|
                ),
            )
            .await?;
        Ok(self)
    }

    pub async fn modify_series(
        &self,
        session: &str,
        series_id: &str,
        series_version: &str,
        series_symbol_id: &str,
        config: &ChartOptions,
    ) -> Result<&Self> {
        let range = match (&config.range, config.from, config.to) {
            (Some(range), _, _) => range.clone(),
            (None, Some(from), Some(to)) => format!("r,{}:{}", from, to),
            _ => String::default(),
        };
        self.socket
            .send(
                "modify_series",
                &payload!(
                    session,
                    series_id,
                    series_version,
                    series_symbol_id,
                    config.interval.to_string(),
                    config.bar_count,
                    range // |r,1626220800:1628640000|1D|5d|1M|3M|6M|YTD|12M|60M|ALL|
                ),
            )
            .await?;
        Ok(self)
    }

    pub async fn remove_series(&self, session: &str, series_id: &str) -> Result<&Self> {
        self.socket
            .send("remove_series", &payload!(session, series_id))
            .await?;
        Ok(self)
    }

    pub async fn resolve_symbol(
        &self,
        session: &str,
        symbol_series_id: &str,
        symbol: &str,
        config: &ChartOptions,
        replay_session: Option<String>,
    ) -> Result<&Self> {
        self.socket
            .send(
                "resolve_symbol",
                &payload!(
                    session,
                    symbol_series_id,
                    symbol_init(
                        symbol,
                        config.adjustment.clone(),
                        config.currency,
                        config.session_type.clone(),
                        replay_session
                    )?
                ),
            )
            .await?;
        Ok(self)
    }

    pub async fn delete(&self) -> Result<&Self> {
        for (_, s) in self.client.read().await.metadata.series.clone() {
            debug!("delete series: {:?}", s);
            self.delete_chart_session_id(&s.chart_session).await?;
        }
        self.socket.close().await?;
        Ok(self)
    }

    // End TradingView WebSocket methods

    pub async fn set_replay(
        &self,
        symbol: &str,
        options: &ChartOptions,
        chart_session: &str,
        symbol_series_id: &str,
    ) -> Result<&Self> {
        let replay_series_id = gen_id();
        let replay_session = gen_session_id("rs");

        self.create_replay_session(&replay_session).await?;
        self.add_replay_series(&replay_session, &replay_series_id, symbol, options)
            .await?;
        self.replay_reset(&replay_session, &replay_series_id, options.replay_from)
            .await?;
        self.resolve_symbol(
            chart_session,
            symbol_series_id,
            &options.symbol,
            options,
            Some(replay_session.to_string()),
        )
        .await?;

        Ok(self)
    }

    pub async fn set_study(
        &self,
        study: &StudyOptions,
        chart_session: &str,
        series_id: &str,
    ) -> Result<&Self> {
        self.client.write().await.metadata.studies_count += 1;
        let study_count = self.client.read().await.metadata.studies_count;
        let study_id = format!("st{}", study_count);

        let indicator = PineIndicator::build()
            .fetch(
                &study.script_id,
                &study.script_version,
                study.script_type.clone(),
            )
            .await?;

        self.client
            .write()
            .await
            .metadata
            .studies
            .insert(indicator.metadata.data.id.clone(), study_id.clone());

        self.create_study(chart_session, &study_id, series_id, indicator)
            .await?;
        Ok(self)
    }

    pub async fn set_market(&self, options: ChartOptions) -> Result<&Self> {
        self.client.write().await.metadata.series_count += 1;
        let series_count = self.client.read().await.metadata.series_count;
        let symbol_series_id = format!("sds_sym_{}", series_count);
        let series_id = format!("sds_{}", series_count);
        let series_version = format!("s{}", series_count);
        let chart_session = gen_session_id("cs");
        let symbol = format!("{}:{}", options.exchange, options.symbol);
        self.create_chart_session(&chart_session).await?;

        if options.replay_mode {
            self.set_replay(&symbol, &options, &chart_session, &symbol_series_id)
                .await?;
        } else {
            self.resolve_symbol(&chart_session, &symbol_series_id, &symbol, &options, None)
                .await?;
        }

        self.create_series(
            &chart_session,
            &series_id,
            &series_version,
            &symbol_series_id,
            &options,
        )
        .await?;

        if let Some(study) = &options.study_config {
            self.set_study(study, &chart_session, &series_id).await?;
        }

        let series_info = SeriesInfo {
            chart_session,
            options,
        };

        self.client
            .write()
            .await
            .metadata
            .series
            .insert(series_id, series_info);

        Ok(self)
    }

    pub async fn subscribe(&self) {
        self.event_loop(&self.socket.to_owned()).await;
    }
}

#[async_trait::async_trait]
impl Socket for WebSocket {
    async fn handle_message_data(&self, message: SocketMessageDe) -> Result<()> {
        let event = TradingViewDataEvent::from(message.m.to_owned());
        self.client
            .write()
            .await
            .handle_events(event, &message.p)
            .await;
        Ok(())
    }

    async fn handle_error(&self, error: Error) {
        (self.client.read().await.callbacks.on_error)((error, vec![]));
    }
}

impl WebSocketClient {
    pub(crate) async fn handle_events(
        &mut self,
        event: TradingViewDataEvent,
        message: &Vec<Value>,
    ) {
        match event {
            TradingViewDataEvent::OnChartData | TradingViewDataEvent::OnChartDataUpdate => {
                trace!("received raw chart data: {:?}", message);
                match self
                    .handle_chart_data(&self.metadata.series, &self.metadata.studies, message)
                    .await
                {
                    Ok(_) => (),
                    Err(e) => {
                        error!("chart data parsing error: {:?}", e);
                        (self.callbacks.on_error)((e, message.to_owned()));
                    }
                };
            }
            TradingViewDataEvent::OnQuoteData => self.handle_quote_data(message).await,
            TradingViewDataEvent::OnSymbolResolved => {
                match SymbolInfo::deserialize(&message[2]) {
                    Ok(s) => {
                        debug!("receive symbol info: {:?}", s);
                        (self.callbacks.on_symbol_info)(s);
                    }
                    Err(e) => {
                        error!("symbol resolved parsing error: {:?}", e);
                        (self.callbacks.on_error)((Error::JsonParseError(e), message.to_owned()));
                    }
                };
            }
            TradingViewDataEvent::OnSeriesCompleted => {
                debug!("series completed: {:?}", message);
                (self.callbacks.on_series_completed)(message.to_owned());
            }
            TradingViewDataEvent::OnSeriesLoading => {
                debug!("series loading: {:?}", message);
                (self.callbacks.on_series_loading)(message.to_owned());
            }
            TradingViewDataEvent::OnQuoteCompleted => {
                debug!("quote completed: {:?}", message);
                (self.callbacks.on_series_completed)(message.to_owned());
            }
            TradingViewDataEvent::OnReplayOk => {
                debug!("replay ok: {:?}", message);
                (self.callbacks.on_series_completed)(message.to_owned());
            }
            TradingViewDataEvent::OnReplayPoint => {
                debug!("replay point: {:?}", message);
                (self.callbacks.on_series_completed)(message.to_owned());
            }
            TradingViewDataEvent::OnReplayInstanceId => {
                debug!("replay instance id: {:?}", message);
                (self.callbacks.on_series_completed)(message.to_owned());
            }
            TradingViewDataEvent::OnReplayResolutions => {
                debug!("replay resolutions: {:?}", message);
                (self.callbacks.on_series_completed)(message.to_owned());
            }
            TradingViewDataEvent::OnReplayDataEnd => {
                debug!("replay data end: {:?}", message);
                (self.callbacks.on_series_completed)(message.to_owned());
            }
            TradingViewDataEvent::OnStudyLoading => {
                debug!("study loading: {:?}", message);
                (self.callbacks.on_study_loading)(message.to_owned());
            }
            TradingViewDataEvent::OnStudyCompleted => {
                debug!("study completed: {:?}", message);
                (self.callbacks.on_study_completed)(message.to_owned());
            }
            TradingViewDataEvent::OnError(trading_view_error) => {
                error!("trading view error: {:?}", trading_view_error);
                (self.callbacks.on_error)((
                    Error::TradingViewError(trading_view_error),
                    message.to_owned(),
                ));
            }
            TradingViewDataEvent::UnknownEvent(event) => {
                warn!("unknown event: {:?}", event);
                (self.callbacks.on_unknown_event)((event, message.to_owned()));
            }
        }
    }

    async fn handle_chart_data(
        &self,
        series: &HashMap<String, SeriesInfo>,
        studies: &HashMap<String, String>,
        message: &[Value],
    ) -> Result<()> {
        for (id, s) in series.iter() {
            debug!("received raw message - v: {:?}, m: {:?}", s, message);
            match message[1].get(id.as_str()) {
                Some(resp_data) => {
                    let data = ChartResponseData::deserialize(resp_data)?.series;
                    self.metadata.chart_data_cache.write().await.replace(data);
                    debug!(
                        "series data extracted: {:?}",
                        self.metadata.chart_data_cache.read().await
                    );
                    (self.callbacks.on_chart_data)((
                        s.options.clone(),
                        self.metadata
                            .chart_data_cache
                            .read()
                            .await
                            .clone()
                            .unwrap_or_default(),
                    ));
                }
                None => {
                    debug!("receive empty data on series: {:?}", s);
                }
            }

            if let Some(study_options) = &s.options.study_config {
                self.handle_study_data(study_options, studies, message)
                    .await?;
            }
        }
        Ok(())
    }

    async fn handle_study_data(
        &self,
        options: &StudyOptions,
        studies: &HashMap<String, String>,
        message: &[Value],
    ) -> Result<()> {
        for (k, v) in studies.iter() {
            if let Some(resp_data) = message[1].get(v.as_str()) {
                debug!("study data received: {} - {:?}", k, resp_data);
                let data = StudyResponseData::deserialize(resp_data)?;
                (self.callbacks.on_study_data)((options.clone(), data));
            }
        }
        Ok(())
    }

    async fn handle_quote_data(&mut self, message: &[Value]) {
        debug!("received raw quote data: {:?}", message);
        let qsd = QuoteData::deserialize(&message[1]).unwrap_or_default();
        if qsd.status == "ok" {
            if let Some(prev_quote) = self.metadata.quotes.get_mut(&qsd.name) {
                *prev_quote = merge_quotes(prev_quote, &qsd.value);
            } else {
                self.metadata.quotes.insert(qsd.name, qsd.value);
            }

            for q in self.metadata.quotes.values() {
                debug!("quote data: {:?}", q);
                (self.callbacks.on_quote_data)(q.to_owned());
            }
        } else {
            error!("quote data status error: {:?}", qsd);
            (self.callbacks.on_error)((
                Error::TradingViewError(TradingViewError::QuoteDataStatusError),
                message.to_owned(),
            ));
        }
    }

    pub fn set_callbacks(mut self, callbacks: Callbacks) -> Self {
        self.callbacks = callbacks;
        self
    }
}
