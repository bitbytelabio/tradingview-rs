use crate::{
    DataPoint, Error, Interval, Result, Timezone,
    chart::{ChartOptions, ChartResponseData, StudyOptions, StudyResponseData, SymbolInfo},
    error::TradingViewError,
    handler::event::TradingViewEventHandlers,
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
use dashmap::DashMap;
use futures_util::future::join_all;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::{
    Arc, OnceLock,
    atomic::{AtomicU16, Ordering},
};
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, error, trace, warn};
use ustr::Ustr;

// Global object pool for reusing Vec<Value>
static PAYLOAD_POOL: OnceLock<Mutex<Vec<Vec<Value>>>> = OnceLock::new();

fn get_payload_pool() -> &'static Mutex<Vec<Vec<Value>>> {
    PAYLOAD_POOL.get_or_init(|| Mutex::new(Vec::new()))
}

async fn get_pooled_payload() -> Vec<Value> {
    let mut pool = get_payload_pool().lock().await;
    pool.pop().unwrap_or_else(|| Vec::with_capacity(8))
}

async fn return_pooled_payload(mut payload: Vec<Value>) {
    payload.clear();
    if payload.capacity() <= 32 {
        // Prevent memory bloat
        let mut pool = get_payload_pool().lock().await;
        if pool.len() < 10 {
            // Limit pool size
            pool.push(payload);
        }
    }
}

#[derive(Clone, Default)]
pub struct WebSocketHandler {
    metadata: Metadata,
    event_handler: TradingViewEventHandlers,
}

#[derive(Default, Clone)]
struct Metadata {
    series_count: Arc<AtomicU16>,
    studies_count: Arc<AtomicU16>,
    series: Arc<DashMap<Ustr, SeriesInfo>>,
    studies: Arc<DashMap<Ustr, Ustr>>,
    quotes: Arc<DashMap<Ustr, QuoteValue>>,
    quote_session: Arc<RwLock<Ustr>>,
    chart_state: Arc<RwLock<ChartState>>,
}

#[derive(Default, Clone)]
struct ChartState {
    chart: Option<(SeriesInfo, Vec<DataPoint>)>,
    symbol_info: Option<SymbolInfo>,
}

#[derive(Clone)]
pub struct WebSocketClient {
    handler: WebSocketHandler,
    socket: SocketSession,
}

#[derive(Default)]
pub struct WebSocketBuilder {
    client: Option<WebSocketHandler>,
    auth_token: Option<Ustr>,
    server: Option<DataServer>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SeriesInfo {
    pub chart_session: Ustr,
    pub options: ChartOptions,
}

impl WebSocketBuilder {
    pub fn client(mut self, client: WebSocketHandler) -> Self {
        self.client = Some(client);
        self
    }

    pub fn auth_token(mut self, auth_token: &str) -> Self {
        self.auth_token = Some(Ustr::from(auth_token));
        self
    }

    pub fn server(mut self, server: DataServer) -> Self {
        self.server = Some(server);
        self
    }

    pub async fn build(self) -> Result<WebSocketClient> {
        let auth_token = self.auth_token.unwrap_or("unauthorized_user_token".into());
        let server = self.server.unwrap_or_default();

        let socket = SocketSession::new(server, auth_token).await?;
        let client = self.client.unwrap_or_default();

        Ok(WebSocketClient::new_with_session(client, socket))
    }
}

impl WebSocketClient {
    pub fn builder() -> WebSocketBuilder {
        WebSocketBuilder::default()
    }

    pub fn new_with_session(client: WebSocketHandler, socket: SocketSession) -> Self {
        Self {
            handler: client,
            socket,
        }
    }

    pub async fn is_closed(&self) -> bool {
        self.socket.is_closed().await
    }

    pub async fn send_batch(&self, messages: Vec<(&str, &Vec<Value>)>) -> Result<&Self> {
        for (method, payload) in messages {
            self.socket.send(method, payload).await?;
        }
        Ok(self)
    }

    pub async fn add_symbols(&self, symbols: &[&str]) -> Result<&Self> {
        let quote_session = self.handler.metadata.quote_session.read().await;
        let mut payloads = get_pooled_payload().await;
        payloads.push(Value::from(quote_session.as_str()));
        payloads.extend(symbols.iter().map(|&s| Value::from(s)));

        let result = self.socket.send("quote_add_symbols", &payloads).await;
        return_pooled_payload(payloads).await;
        result?;
        Ok(self)
    }

    // Begin TradingView WebSocket Quote methods
    pub async fn create_quote_session(&self) -> Result<&Self> {
        let mut quote_session = self.handler.metadata.quote_session.write().await;
        *quote_session = Ustr::from(&gen_session_id("qs"));
        self.socket
            .send("quote_create_session", &payload!(quote_session.to_string()))
            .await?;
        Ok(self)
    }

    pub async fn delete_quote_session(&self) -> Result<&Self> {
        let quote_session = self.handler.metadata.quote_session.read().await;
        self.socket
            .send("quote_delete_session", &payload!(quote_session.to_string()))
            .await?;
        Ok(self)
    }

    pub async fn set_fields(&self) -> Result<&Self> {
        let quote_session = self.handler.metadata.quote_session.read().await;
        let mut quote_fields = get_pooled_payload().await;
        quote_fields.push(Value::from(quote_session.to_string()));
        quote_fields.extend(ALL_QUOTE_FIELDS.clone().into_iter().map(Value::from));

        let result = self.socket.send("quote_set_fields", &quote_fields).await;
        return_pooled_payload(quote_fields).await;
        result?;
        Ok(self)
    }

    pub async fn set_auth_token(&mut self, auth_token: &str) -> Result<&mut Self> {
        self.socket.set_auth_token(auth_token).await?;
        Ok(self)
    }

    pub async fn fast_symbols(&self, symbols: Vec<&str>) -> Result<&Self> {
        let quote_session = self.handler.metadata.quote_session.read().await;
        let mut payloads = get_pooled_payload().await;
        payloads.push(Value::from(quote_session.to_string()));
        payloads.extend(symbols.into_iter().map(Value::from));

        let result = self.socket.send("quote_fast_symbols", &payloads).await;
        return_pooled_payload(payloads).await;
        result?;
        Ok(self)
    }

    pub async fn remove_symbols(&self, symbols: Vec<&str>) -> Result<&Self> {
        let quote_session = self.handler.metadata.quote_session.read().await;
        let mut payloads = get_pooled_payload().await;
        payloads.push(Value::from(quote_session.to_string()));
        payloads.extend(symbols.into_iter().map(Value::from));

        let result = self.socket.send("quote_remove_symbols", &payloads).await;
        return_pooled_payload(payloads).await;
        result?;
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
        config: ChartOptions,
    ) -> Result<&Self> {
        self.socket
            .send(
                "replay_add_series",
                &payload!(
                    session,
                    series_id,
                    symbol_init(
                        symbol,
                        config.adjustment,
                        config.currency,
                        config.session_type,
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
        let mut payloads = get_pooled_payload().await;
        payloads.extend([
            Value::from(session),
            Value::from(study_id),
            Value::from("st1"),
            Value::from(series_id),
            Value::from(indicator.script_type.to_string()),
            inputs,
        ]);

        let result = self.socket.send("create_study", &payloads).await;
        return_pooled_payload(payloads).await;
        result?;
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
        let mut payloads = get_pooled_payload().await;
        payloads.extend([
            Value::from(session),
            Value::from(study_id),
            Value::from("st1"),
            Value::from(series_id),
            Value::from(indicator.script_type.to_string()),
            inputs,
        ]);

        let result = self.socket.send("modify_study", &payloads).await;
        return_pooled_payload(payloads).await;
        result?;
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
        config: ChartOptions,
    ) -> Result<&Self> {
        let range = match (&config.range, config.from, config.to) {
            (Some(range), _, _) => range.clone(),
            (None, Some(from), Some(to)) => Ustr::from(&format!("r,{from}:{to}")),
            _ => Ustr::default(),
        };

        let mut payloads = get_pooled_payload().await;
        payloads.extend([
            Value::from(session),
            Value::from(series_id),
            Value::from(series_version),
            Value::from(series_symbol_id),
            Value::from(config.interval.to_string()),
            Value::from(config.bar_count),
            Value::from(range.to_string()),
        ]);

        let result = self.socket.send("create_series", &payloads).await;
        return_pooled_payload(payloads).await;
        result?;
        Ok(self)
    }

    pub async fn modify_series(
        &self,
        session: &str,
        series_id: &str,
        series_version: &str,
        series_symbol_id: &str,
        config: ChartOptions,
    ) -> Result<&Self> {
        let range = match (&config.range, config.from, config.to) {
            (Some(range), _, _) => range.clone(),
            (None, Some(from), Some(to)) => Ustr::from(&format!("r,{from}:{to}")),
            _ => Ustr::default(),
        };

        let mut payloads = get_pooled_payload().await;
        payloads.extend([
            Value::from(session),
            Value::from(series_id),
            Value::from(series_version),
            Value::from(series_symbol_id),
            Value::from(config.interval.to_string()),
            Value::from(config.bar_count),
            Value::from(range.to_string()),
        ]);

        let result = self.socket.send("modify_series", &payloads).await;
        return_pooled_payload(payloads).await;
        result?;
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
        config: ChartOptions,
        replay_session: Option<&str>,
    ) -> Result<&Self> {
        self.socket
            .send(
                "resolve_symbol",
                &payload!(
                    session,
                    symbol_series_id,
                    symbol_init(
                        symbol,
                        config.adjustment,
                        config.currency,
                        config.session_type,
                        replay_session
                    )?
                ),
            )
            .await?;
        Ok(self)
    }

    pub async fn delete(&self) -> Result<&Self> {
        // Collect all sessions first to avoid holding iterator while making async calls
        let chart_sessions: Vec<Ustr> = self
            .handler
            .metadata
            .series
            .iter()
            .map(|entry| entry.value().chart_session.clone())
            .collect();

        // Delete quote session first
        if let Err(e) = self.delete_quote_session().await {
            warn!("Failed to delete quote session: {:?}", e);
        }

        // Delete all chart sessions in parallel for better performance
        let delete_futures: Vec<_> = chart_sessions
            .iter()
            .map(|session| self.delete_chart_session_id(session))
            .collect();

        // Execute all deletions concurrently
        let results = join_all(delete_futures).await;

        // Log any errors but don't fail the entire operation
        for (i, result) in results.into_iter().enumerate() {
            if let Err(e) = result {
                warn!(
                    "Failed to delete chart session {}: {:?}",
                    chart_sessions[i], e
                );
            }
        }

        // Clear all metadata
        self.handler.metadata.series.clear();
        self.handler.metadata.studies.clear();
        self.handler.metadata.quotes.clear();

        // Reset counters
        self.handler
            .metadata
            .series_count
            .store(0, Ordering::SeqCst);
        self.handler
            .metadata
            .studies_count
            .store(0, Ordering::SeqCst);

        // Close the socket last
        if let Err(e) = self.socket.close().await {
            error!("Failed to close socket: {:?}", e);
            return Err(e);
        }

        debug!("WebSocket client deleted successfully");
        Ok(self)
    }

    // End TradingView WebSocket methods

    pub async fn set_replay(
        &self,
        symbol: &str,
        options: ChartOptions,
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
            Some(&replay_session),
        )
        .await?;

        Ok(self)
    }

    pub async fn set_study(
        &self,
        study: StudyOptions,
        chart_session: &str,
        series_id: &str,
    ) -> Result<&Self> {
        let study_count = self
            .handler
            .metadata
            .studies_count
            .fetch_add(1, Ordering::SeqCst)
            + 1;

        let study_id = Ustr::from(&format!("st{study_count}"));

        let indicator = PineIndicator::build()
            .fetch(
                &study.script_id,
                &study.script_version,
                study.script_type.clone(),
            )
            .await?;

        let key = Ustr::from(&indicator.metadata.data.id);

        self.handler.metadata.studies.insert(key, study_id);

        self.create_study(chart_session, &study_id, series_id, indicator)
            .await?;
        Ok(self)
    }

    pub async fn set_market(&self, options: ChartOptions) -> Result<&Self> {
        let series_count = self
            .handler
            .metadata
            .series_count
            .fetch_add(1, Ordering::SeqCst)
            + 1;
        let symbol_series_id = format!("sds_sym_{series_count}");
        let series_id = Ustr::from(&format!("sds_{series_count}"));
        let series_version = format!("s{series_count}");
        let chart_session = Ustr::from(&gen_session_id("cs"));
        let symbol = format!("{}:{}", options.exchange, options.symbol);
        self.create_chart_session(&chart_session).await?;

        if options.replay_mode {
            self.set_replay(&symbol, options, &chart_session, &symbol_series_id)
                .await?;
        } else {
            self.resolve_symbol(&chart_session, &symbol_series_id, &symbol, options, None)
                .await?;
        }

        self.create_series(
            &chart_session,
            &series_id,
            &series_version,
            &symbol_series_id,
            options,
        )
        .await?;

        if let Some(study) = options.study_config {
            self.set_study(study, &chart_session, &series_id).await?;
        }

        let series_info = SeriesInfo {
            chart_session,
            options,
        };

        self.handler.metadata.series.insert(series_id, series_info);

        Ok(self)
    }

    pub async fn subscribe(&self) {
        self.event_loop(&self.socket.to_owned()).await;
    }

    pub async fn reconnect(&mut self) -> Result<()> {
        self.socket.reconnect().await?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl Socket for WebSocketClient {
    async fn handle_message_data(&self, message: SocketMessageDe) -> Result<()> {
        let event = TradingViewDataEvent::from(message.m.to_owned());
        self.handler.handle_events(event, &message.p).await;
        Ok(())
    }

    async fn handle_error(&self, error: Error) {
        (self.handler.event_handler.on_error)((error, vec![]));
    }
}

impl WebSocketHandler {
    pub(crate) async fn handle_events(&self, event: TradingViewDataEvent, message: &Vec<Value>) {
        match event {
            TradingViewDataEvent::OnChartData | TradingViewDataEvent::OnChartDataUpdate => {
                trace!("received raw chart data: {:?}", message);
                // Avoid cloning the entire HashMap
                if let Err(e) = self.handle_chart_data(message).await {
                    error!("chart data parsing error: {:?}", e);
                    (self.event_handler.on_error)((e, message.to_owned()));
                }
            }
            TradingViewDataEvent::OnQuoteData => self.handle_quote_data(message).await,
            TradingViewDataEvent::OnSymbolResolved => {
                if message.len() > 2 {
                    match SymbolInfo::deserialize(&message[2]) {
                        Ok(s) => {
                            debug!("receive symbol info: {:?}", s);
                            let mut chart_state = self.metadata.chart_state.write().await;
                            chart_state.symbol_info.replace(s.clone());
                            (self.event_handler.on_symbol_info)(s);
                        }
                        Err(e) => {
                            error!("symbol resolved parsing error: {:?}", e);
                            (self.event_handler.on_error)((
                                Error::JsonParseError(e),
                                message.to_owned(),
                            ));
                        }
                    }
                }
            }
            TradingViewDataEvent::OnSeriesCompleted => {
                debug!("series completed: {:?}", message);
                (self.event_handler.on_series_completed)(message.to_owned());
            }
            TradingViewDataEvent::OnSeriesLoading => {
                debug!("series loading: {:?}", message);
                (self.event_handler.on_series_loading)(message.to_owned());
            }
            TradingViewDataEvent::OnQuoteCompleted => {
                debug!("quote completed: {:?}", message);
                (self.event_handler.on_quote_completed)(message.to_owned());
            }
            TradingViewDataEvent::OnReplayOk => {
                debug!("replay ok: {:?}", message);
                (self.event_handler.on_replay_ok)(message.to_owned());
            }
            TradingViewDataEvent::OnReplayPoint => {
                debug!("replay point: {:?}", message);
                (self.event_handler.on_replay_point)(message.to_owned());
            }
            TradingViewDataEvent::OnReplayInstanceId => {
                debug!("replay instance id: {:?}", message);
                (self.event_handler.on_replay_instance_id)(message.to_owned());
            }
            TradingViewDataEvent::OnReplayResolutions => {
                debug!("replay resolutions: {:?}", message);
                (self.event_handler.on_replay_resolutions)(message.to_owned());
            }
            TradingViewDataEvent::OnReplayDataEnd => {
                debug!("replay data end: {:?}", message);
                (self.event_handler.on_replay_data_end)(message.to_owned());
            }
            TradingViewDataEvent::OnStudyLoading => {
                debug!("study loading: {:?}", message);
                (self.event_handler.on_study_loading)(message.to_owned());
            }
            TradingViewDataEvent::OnStudyCompleted => {
                debug!("study completed: {:?}", message);
                (self.event_handler.on_study_completed)(message.to_owned());
            }
            TradingViewDataEvent::OnError(tradingview_error) => {
                error!("trading view error: {:?}", tradingview_error);
                (self.event_handler.on_error)((
                    Error::TradingViewError(tradingview_error),
                    message.to_owned(),
                ));
            }
            TradingViewDataEvent::UnknownEvent(event) => {
                warn!("unknown event: {:?}", event);
                (self.event_handler.on_unknown_event)((event, message.to_owned()));
            }
        }
    }

    async fn handle_study_data(&self, options: &StudyOptions, message_data: &Value) -> Result<()> {
        for study_entry in self.metadata.studies.iter() {
            let (k, v) = study_entry.pair();
            if let Some(resp_data) = message_data.get(v.as_str()) {
                debug!("study data received: {} - {:?}", k, resp_data);
                let data = StudyResponseData::deserialize(resp_data)?;
                (self.event_handler.on_study_data)((options.clone(), data));
            }
        }
        Ok(())
    }

    async fn handle_chart_data(&self, message: &[Value]) -> Result<()> {
        if message.len() < 2 {
            return Ok(());
        }

        let message_data = &message[1];

        // Process each series without cloning the entire series map
        for series_entry in self.metadata.series.iter() {
            let (id, series_info) = series_entry.pair();

            if let Some(resp_data) = message_data.get(id.as_str()) {
                let data = ChartResponseData::deserialize(resp_data)?.series;

                // Update chart state
                {
                    let mut chart_state = self.metadata.chart_state.write().await;
                    chart_state
                        .chart
                        .replace((series_info.clone(), data.clone()));
                }

                // Read once for callback
                let chart_data = self
                    .metadata
                    .chart_state
                    .read()
                    .await
                    .chart
                    .clone()
                    .unwrap_or_default();
                (self.event_handler.on_chart_data)(chart_data);

                // Handle study data if present
                if let Some(study_options) = &series_info.options.study_config {
                    self.handle_study_data(study_options, message_data).await?;
                }
            }
        }

        Ok(())
    }

    async fn handle_quote_data(&self, message: &[Value]) {
        if message.len() < 2 {
            return;
        }

        debug!("received raw quote data: {:?}", message);
        let qsd = match QuoteData::deserialize(&message[1]) {
            Ok(data) => data,
            Err(_) => return,
        };

        if qsd.status == "ok" {
            // Use DashMap's efficient upsert
            self.metadata
                .quotes
                .entry(qsd.name)
                .and_modify(|prev_quote| {
                    *prev_quote = merge_quotes(prev_quote, &qsd.value);
                })
                .or_insert(qsd.value);

            // Collect quotes once to avoid multiple read locks
            let quotes: Vec<QuoteValue> = self
                .metadata
                .quotes
                .iter()
                .map(|entry| entry.value().clone())
                .collect();
            for quote in quotes {
                (self.event_handler.on_quote_data)(quote);
            }
        } else {
            error!("quote data status error: {:?}", qsd);
            (self.event_handler.on_error)((
                Error::TradingViewError(TradingViewError::QuoteDataStatusError),
                message.to_owned(),
            ));
        }
    }

    pub fn set_callbacks(mut self, callbacks: TradingViewEventHandlers) -> Self {
        self.event_handler = callbacks;
        self
    }
}
