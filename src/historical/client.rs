#![allow(deprecated)]
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;
use tracing::{debug, info, instrument, warn};

use crate::{
    DataPoint, DataServer, Error, Result, SymbolInfo,
    historical::{HistoricalRequest, HistoricalResult, state::HistoricalState},
    live::handler::CommandTx,
    live::handler::LegacyHandler,
    live::models::TradingViewDataEvent,
    live::websocket::WebSocketClient,
};
use serde_json::Value;

/// High-level client for fetching historical TradingView chart data.
pub struct HistoricalClient {
    auth_token: String,
    server: DataServer,
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
        let handler = HistoricalDataHandler::new(state.clone(), cmd_tx);

        let ws = WebSocketClient::builder()
            .auth_token(&self.auth_token)
            .server(self.server)
            .handler(handler)
            .build()
            .await?;

        // Resolve symbol — tells TradingView which instrument we want.
        let instrument = format!("{exchange}:{symbol}");
        ws.send("resolve_symbol", &[Value::from(instrument.as_str())])
            .await?;
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

        let mut state_guard = state.lock().await;
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
            let guard = state.lock().await;
            if guard.completed || guard.errored {
                break;
            }
        }
    }
}

// =============================================================================
// HistoricalDataHandler
// =============================================================================

#[derive(Clone)]
struct HistoricalDataHandler {
    state: Arc<Mutex<HistoricalState>>,
    #[allow(dead_code)]
    cmd_tx: CommandTx,
}

impl HistoricalDataHandler {
    fn new(state: Arc<Mutex<HistoricalState>>, cmd_tx: CommandTx) -> Self {
        Self { state, cmd_tx }
    }
}

#[allow(deprecated)]
#[allow(deprecated)]
impl LegacyHandler for HistoricalDataHandler {
    fn new(_command_tx: CommandTx) -> Self {
        unreachable!()
    }

    fn handle_events(&self, event: TradingViewDataEvent, message: &[Value]) {
        match event {
            TradingViewDataEvent::OnSymbolResolved => {
                if let Some(sym_info) = message.first() {
                    if let Ok(info) = serde_json::from_value::<SymbolInfo>(sym_info.clone()) {
                        debug!(name = %info.name, "Symbol resolved");
                        self.state.blocking_lock().record_symbol_info(info);
                    }
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
                                let mut state = self.state.blocking_lock();
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
                self.state.blocking_lock().complete();
            }
            _ => {}
        }
    }

    fn handle_quote_data(&self, _message: &[Value]) {}
    fn handle_series_data(&self, _event: TradingViewDataEvent, _messages: &[Value]) {}
    fn notify_error(&self, error: Error, _message: &[Value]) {
        warn!(?error, "Historical handler error");
        let mut state = self.state.blocking_lock();
        if state.record_error() {
            state.fail(format!("Too many errors: {error:?}"));
        }
    }
}
