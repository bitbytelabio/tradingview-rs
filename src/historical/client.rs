use std::sync::{Arc, Mutex};
use std::time::Instant;
use tracing::{debug, info, instrument, warn};

use crate::{
    DataPoint, DataServer, Error, Result, SymbolInfo,
    historical::{HistoricalRequest, HistoricalResult, state::HistoricalState},
    live::handler::{CommandTx, Handler, HandlerFactory},
    live::models::TradingViewDataEvent,
    live::websocket::WebSocketClient,
    utils::symbol_init,
};
use serde_json::Value;
use tracing::error;

/// High-level client for fetching historical TradingView chart data.
pub struct HistoricalClient {
    pub(crate) auth_token: String,
    pub(crate) server: DataServer,
}

impl HistoricalClient {
    pub fn new(auth_token: impl Into<String>, server: DataServer) -> Self {
        Self {
            auth_token: auth_token.into(),
            server,
        }
    }

    #[instrument(skip(self), fields(symbol, exchange))]
    pub async fn retrieve(&self, request: HistoricalRequest) -> Result<HistoricalResult> {
        let started = Instant::now();
        let (symbol, exchange) = request.resolve_symbol_exchange()?;
        debug!(symbol = %symbol, exchange = %exchange, "Historical retrieval started");

        let state = Arc::new(Mutex::new(if let Some(n) = request.num_bars {
            HistoricalState::with_capacity(n as usize)
        } else {
            HistoricalState::new()
        }));

        let (cmd_tx, _cmd_rx) =
            tokio::sync::mpsc::channel::<crate::live::handler::command::Command>(16);
        let factory = HistoricalDataHandlerFactory::new(state.clone());
        let handler = factory.create(cmd_tx);

        let ws = WebSocketClient::builder()
            .auth_token(&self.auth_token)
            .server(self.server)
            .handler(handler)
            .build()
            .await?;

        // ── Protocol sequence ──────────────────────────────────────────
        // TradingView chart data protocol:
        //   1. chart_create_session → server acknowledges with session
        //   2. resolve_symbol       → server returns SymbolInfo
        //   3. create_series        → server starts streaming chart data
        //   4. OnSeriesCompleted    → all data received

        let instrument = format!("{exchange}:{symbol}");
        let chart_session = format!("cs_{}", crate::utils::gen_id());
        let symbol_series_id = format!("sds_sym_{}", crate::utils::gen_id());
        let series_identifier = "sds_1".to_string();
        let series_id = "s1".to_string();

        // 1. Create chart session.
        ws.send(
            "chart_create_session",
            &[Value::from(chart_session.as_str())],
        )
        .await?;
        debug!(session = %chart_session, "Chart session created");

        // 2. Resolve symbol within the session.
        let symbol_init_str = symbol_init().instrument(&instrument).call()?;
        ws.send(
            "resolve_symbol",
            &[
                Value::from(chart_session.as_str()),
                Value::from(symbol_series_id.as_str()),
                Value::from(symbol_init_str),
            ],
        )
        .await?;
        debug!(instrument = %instrument, "Symbol resolution requested");

        // 3. Create data series to start receiving chart data.
        let bar_count = request.num_bars.unwrap_or(100);
        ws.send(
            "create_series",
            &[
                Value::from(chart_session.as_str()),
                Value::from(series_identifier.as_str()),
                Value::from(series_id.as_str()),
                Value::from(symbol_series_id.as_str()),
                Value::from(request.interval.to_string()),
                Value::from(bar_count),
                Value::from(""),
            ],
        )
        .await?;
        debug!(
            interval = ?request.interval,
            bars = bar_count,
            "Data series created"
        );

        // Also set up a quote session for supplementary data.
        let qs = format!("qs_{}", crate::utils::gen_id());
        ws.send("quote_create_session", &[Value::from(qs.as_str())])
            .await?;
        ws.send("quote_set_fields", &[Value::from(qs.as_str())])
            .await?;
        ws.send(
            "quote_add_symbols",
            &[Value::from(qs.as_str()), Value::from(symbol.as_str())],
        )
        .await?;

        Arc::clone(&ws).spawn_reader_task();

        let result = tokio::time::timeout(request.timeout, Self::wait_for_completion(&state)).await;

        let mut state_guard = state.lock().unwrap();
        let total_bars = state_guard.total_bars;
        let data = state_guard.finalize();
        let elapsed = started.elapsed();

        match result {
            Ok(_) => {
                let symbol_info = state_guard
                    .symbol_info
                    .take()
                    .ok_or_else(|| Error::Internal("No symbol info received".into()))?;
                Ok(HistoricalResult {
                    symbol_info,
                    data,
                    series_info: state_guard.series_info.take(),
                    total_bars_received: total_bars,
                    replay_used: request.with_replay,
                    elapsed,
                })
            }
            Err(_) => Err(Error::Timeout("Historical data retrieval timed out".into())),
        }
    }

    async fn wait_for_completion(state: &Arc<Mutex<HistoricalState>>) {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            let guard = state.lock().unwrap();
            if guard.completed || guard.errored {
                break;
            }
        }
    }
}

// =============================================================================
// HistoricalDataHandler
// =============================================================================

/// Event handler that accumulates chart data points into shared
/// [`HistoricalState`].  Implements the [`Handler`] trait for use with
/// [`WebSocketClient`].
#[derive(Clone)]
pub struct HistoricalDataHandler {
    state: Arc<Mutex<HistoricalState>>,
    #[allow(dead_code)]
    cmd_tx: CommandTx,
}

impl Handler for HistoricalDataHandler {
    fn handle_events(&self, event: TradingViewDataEvent, message: &[Value]) {
        match event {
            TradingViewDataEvent::OnSymbolResolved => {
                // resolve_symbol response: [session, symbol_series_id, SymbolInfo]
                if let Some(sym_info) = message.get(2)
                    && let Ok(info) = serde_json::from_value::<SymbolInfo>(sym_info.clone())
                {
                    debug!(name = %info.name, "Symbol resolved");
                    self.state.lock().unwrap().record_symbol_info(info);
                }
            }
            TradingViewDataEvent::OnChartData | TradingViewDataEvent::OnChartDataUpdate => {
                if message.len() < 2 {
                    return;
                }
                if let Some(obj) = message[1].as_object() {
                    for (_key, series_val) in obj {
                        if let Some(s_arr) = series_val.get("s").and_then(|v| v.as_array()) {
                            let points: Vec<DataPoint> = s_arr
                                .iter()
                                .filter_map(|v| serde_json::from_value(v.clone()).ok())
                                .collect();
                            if !points.is_empty() {
                                let mut state = self.state.lock().unwrap();
                                state.data.extend(points);
                                state.total_bars += s_arr.len();
                                if state.first_data_at.is_none() {
                                    state.first_data_at = Some(Instant::now());
                                }
                            }
                        }
                    }
                }
            }
            TradingViewDataEvent::OnSeriesCompleted => {
                info!("Series completed");
                self.state.lock().unwrap().complete();
            }
            TradingViewDataEvent::OnError(tv_error) => {
                error!(?tv_error, "TradingView protocol error");
                let mut state = self.state.lock().unwrap();
                state.fail(format!("TradingView error: {tv_error:?}"));
            }
            _ => {}
        }
    }

    fn handle_quote_data(&self, _message: &[Value]) {}
    fn handle_series_data(&self, _event: TradingViewDataEvent, _messages: &[Value]) {}

    fn notify_error(&self, error: Error, _message: &[Value]) {
        warn!(?error, "Historical handler error");
        let mut state = self.state.lock().unwrap();
        if state.record_error() {
            state.fail(format!("Too many errors: {error:?}"));
        }
    }
}

// =============================================================================
// HistoricalDataHandlerFactory
// =============================================================================

/// Factory for creating [`HistoricalDataHandler`] instances that share a
/// common [`HistoricalState`].
pub struct HistoricalDataHandlerFactory {
    state: Arc<Mutex<HistoricalState>>,
}

impl HistoricalDataHandlerFactory {
    /// Create a new factory wrapping the given shared state.
    pub fn new(state: Arc<Mutex<HistoricalState>>) -> Self {
        Self { state }
    }
}

impl HandlerFactory for HistoricalDataHandlerFactory {
    type Handler = HistoricalDataHandler;

    fn create(&self, command_tx: CommandTx) -> Self::Handler {
        HistoricalDataHandler {
            state: Arc::clone(&self.state),
            cmd_tx: command_tx,
        }
    }
}
