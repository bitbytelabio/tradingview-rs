use std::sync::{Arc, Mutex};
use std::time::Instant;
use tracing::{debug, error, info, instrument, warn};

use serde::Deserialize;
use serde_json::Value;

use crate::{
    DataPoint, DataServer, Error, Result, SymbolInfo,
    chart::options::Range,
    historical::{HistoricalRequest, HistoricalResult, state::HistoricalState},
    live::handler::{CommandTx, Handler, HandlerFactory},
    live::models::TradingViewDataEvent,
    live::websocket::WebSocketClient,
    utils::symbol_init,
};

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
        let factory = HistoricalDataHandlerFactory::new(Arc::clone(&state));
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
        ws.create_chart_session(&chart_session).await?;
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
        // In count mode, exactly 6 arguments are sent (no range).
        // In range mode, exactly 7 arguments are sent with bar_count = 0.
        let create_series_args = build_create_series_args(
            &chart_session,
            &series_identifier,
            &series_id,
            &symbol_series_id,
            request.interval,
            request.num_bars,
            request.range,
        );
        ws.send("create_series", &create_series_args).await?;
        debug!(
            interval = ?request.interval,
            bars = request.num_bars,
            range = ?request.range,
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
                if state_guard.errored {
                    let msg = state_guard
                        .error_message
                        .take()
                        .unwrap_or_else(|| "Historical data retrieval failed".to_string());
                    return Err(Error::Internal(msg.into()));
                }
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

    pub(crate) async fn wait_for_completion(state: &Arc<Mutex<HistoricalState>>) {
        let notify = {
            let guard = state.lock().unwrap();
            guard.notify.clone()
        };
        loop {
            // Register as waiter before checking predicate to avoid lost wakeups.
            let notified = notify.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();

            {
                let guard = state.lock().unwrap();
                if guard.completed || guard.errored {
                    break;
                }
            }

            notified.await;
        }
    }
}

/// Builds the argument list for TradingView's `create_series` WebSocket message.
///
/// In count mode (`range` is `None`), the message must have exactly 6 arguments:
/// `[chart_session, series_identifier, series_id, symbol_series_id, interval, bar_count]`.
///
/// In range mode (`range` is `Some`), the message must have exactly 7 arguments:
/// `[chart_session, series_identifier, series_id, symbol_series_id, interval, 0, range]`.
/// Providing 7 arguments with an empty range causes the server to fail with
/// `critical_error: "unsupported method: du"`.
pub(crate) fn build_create_series_args(
    chart_session: &str,
    series_identifier: &str,
    series_id: &str,
    symbol_series_id: &str,
    interval: crate::Interval,
    num_bars: Option<u64>,
    range: Option<Range>,
) -> Vec<Value> {
    if let Some(r) = range {
        vec![
            Value::from(chart_session),
            Value::from(series_identifier),
            Value::from(series_id),
            Value::from(symbol_series_id),
            Value::from(interval.to_string()),
            Value::from(0u64), // bar_count MUST be 0 in range mode
            Value::from(r.to_string()),
        ]
    } else {
        let bar_count = num_bars.unwrap_or(100);
        vec![
            Value::from(chart_session),
            Value::from(series_identifier),
            Value::from(series_id),
            Value::from(symbol_series_id),
            Value::from(interval.to_string()),
            Value::from(bar_count),
        ]
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
                    && let Ok(info) = SymbolInfo::deserialize(sym_info)
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
                            let mut points = Vec::with_capacity(s_arr.len());
                            for v in s_arr {
                                if let Ok(point) = DataPoint::deserialize(v) {
                                    points.push(point);
                                }
                            }
                            if !points.is_empty() {
                                let mut state = self.state.lock().unwrap();
                                state.record_points(points, s_arr.len());
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Interval;
    use crate::chart::options::Range;
    use crate::error::TradingViewError;

    #[test]
    fn test_create_series_args_count_mode_default_bars() {
        let args = build_create_series_args(
            "cs_test",
            "sds_1",
            "s1",
            "sds_sym_1",
            Interval::OneDay,
            None,
            None,
        );
        assert_eq!(
            args.len(),
            6,
            "Count mode without range must have exactly 6 arguments"
        );
        assert_eq!(args[0], Value::from("cs_test"));
        assert_eq!(args[1], Value::from("sds_1"));
        assert_eq!(args[2], Value::from("s1"));
        assert_eq!(args[3], Value::from("sds_sym_1"));
        assert_eq!(args[4], Value::from("1D"));
        assert_eq!(
            args[5],
            Value::from(100u64),
            "Default bar count must be 100"
        );
    }

    #[test]
    fn test_create_series_args_count_mode_custom_bars() {
        let args = build_create_series_args(
            "cs_test",
            "sds_1",
            "s1",
            "sds_sym_1",
            Interval::FiveMinutes,
            Some(500),
            None,
        );
        assert_eq!(
            args.len(),
            6,
            "Count mode without range must have exactly 6 arguments"
        );
        assert_eq!(args[4], Value::from("5"));
        assert_eq!(args[5], Value::from(500u64));
    }

    #[test]
    fn test_create_series_args_range_mode_from_to() {
        let range = Range::FromTo(1626220800, 1628640000);
        let args = build_create_series_args(
            "cs_test",
            "sds_1",
            "s1",
            "sds_sym_1",
            Interval::OneDay,
            Some(500), // even if num_bars is provided, range mode must zero it
            Some(range),
        );
        assert_eq!(args.len(), 7, "Range mode must have exactly 7 arguments");
        assert_eq!(args[0], Value::from("cs_test"));
        assert_eq!(args[1], Value::from("sds_1"));
        assert_eq!(args[2], Value::from("s1"));
        assert_eq!(args[3], Value::from("sds_sym_1"));
        assert_eq!(args[4], Value::from("1D"));
        assert_eq!(
            args[5],
            Value::from(0u64),
            "Bar count in range mode must be 0"
        );
        assert_eq!(args[6], Value::from(range.to_string()));
    }

    #[test]
    fn test_create_series_args_range_mode_preset() {
        let range = Range::OneDay;
        let args = build_create_series_args(
            "cs_test",
            "sds_1",
            "s1",
            "sds_sym_1",
            Interval::OneMinute,
            None,
            Some(range),
        );
        assert_eq!(args.len(), 7, "Range mode must have exactly 7 arguments");
        assert_eq!(
            args[5],
            Value::from(0u64),
            "Bar count in range mode must be 0"
        );
        assert_eq!(args[6], Value::from(range.to_string()));
    }

    #[tokio::test]
    async fn test_wait_for_completion_wakes_immediately_on_complete() {
        let state = Arc::new(Mutex::new(HistoricalState::new()));
        let state_clone = Arc::clone(&state);

        let wait_handle = tokio::spawn(async move {
            HistoricalClient::wait_for_completion(&state_clone).await;
        });

        tokio::task::yield_now().await;

        let start = std::time::Instant::now();
        state.lock().unwrap().complete();

        let timeout_result =
            tokio::time::timeout(std::time::Duration::from_millis(50), wait_handle).await;
        assert!(
            timeout_result.is_ok(),
            "wait_for_completion must wake without polling"
        );
        assert!(start.elapsed() < std::time::Duration::from_millis(50));
        assert!(state.lock().unwrap().completed);
    }

    #[tokio::test]
    async fn test_wait_for_completion_wakes_immediately_on_fail() {
        let state = Arc::new(Mutex::new(HistoricalState::new()));
        let state_clone = Arc::clone(&state);

        let wait_handle = tokio::spawn(async move {
            HistoricalClient::wait_for_completion(&state_clone).await;
        });

        tokio::task::yield_now().await;

        let start = std::time::Instant::now();
        state.lock().unwrap().fail("simulated error".into());

        let timeout_result =
            tokio::time::timeout(std::time::Duration::from_millis(50), wait_handle).await;
        assert!(
            timeout_result.is_ok(),
            "wait_for_completion must wake immediately on failure"
        );
        assert!(start.elapsed() < std::time::Duration::from_millis(50));
        let guard = state.lock().unwrap();
        assert!(guard.errored);
        assert_eq!(guard.error_message.as_deref(), Some("simulated error"));
    }

    #[tokio::test]
    async fn test_wait_for_completion_lost_wakeup_safe_pre_completed() {
        let state = Arc::new(Mutex::new(HistoricalState::new()));

        // Complete state BEFORE calling wait_for_completion
        state.lock().unwrap().complete();

        let result = tokio::time::timeout(
            std::time::Duration::from_millis(20),
            HistoricalClient::wait_for_completion(&state),
        )
        .await;

        assert!(
            result.is_ok(),
            "wait_for_completion must return immediately if already completed"
        );
    }

    #[tokio::test]
    async fn test_wait_for_completion_lost_wakeup_safe_pre_errored() {
        let state = Arc::new(Mutex::new(HistoricalState::new()));

        // Fail state BEFORE calling wait_for_completion
        state.lock().unwrap().fail("early failure".into());

        let result = tokio::time::timeout(
            std::time::Duration::from_millis(20),
            HistoricalClient::wait_for_completion(&state),
        )
        .await;

        assert!(
            result.is_ok(),
            "wait_for_completion must return immediately if already errored"
        );
    }

    #[tokio::test]
    async fn test_handler_signals_series_completed() {
        let state = Arc::new(Mutex::new(HistoricalState::new()));
        let factory = HistoricalDataHandlerFactory::new(Arc::clone(&state));
        let (cmd_tx, _cmd_rx) = tokio::sync::mpsc::channel(4);
        let handler = factory.create(cmd_tx);

        let state_clone = Arc::clone(&state);
        let wait_handle = tokio::spawn(async move {
            HistoricalClient::wait_for_completion(&state_clone).await;
        });

        tokio::task::yield_now().await;

        handler.handle_events(TradingViewDataEvent::OnSeriesCompleted, &[]);

        let timeout_result =
            tokio::time::timeout(std::time::Duration::from_millis(50), wait_handle).await;
        assert!(
            timeout_result.is_ok(),
            "handler must signal completion immediately to waiters"
        );
        assert!(state.lock().unwrap().completed);
    }

    #[tokio::test]
    async fn test_handler_signals_protocol_error() {
        let state = Arc::new(Mutex::new(HistoricalState::new()));
        let factory = HistoricalDataHandlerFactory::new(Arc::clone(&state));
        let (cmd_tx, _cmd_rx) = tokio::sync::mpsc::channel(4);
        let handler = factory.create(cmd_tx);

        let state_clone = Arc::clone(&state);
        let wait_handle = tokio::spawn(async move {
            HistoricalClient::wait_for_completion(&state_clone).await;
        });

        tokio::task::yield_now().await;

        handler.handle_events(
            TradingViewDataEvent::OnError(TradingViewError::SeriesError),
            &[],
        );

        let timeout_result =
            tokio::time::timeout(std::time::Duration::from_millis(50), wait_handle).await;
        assert!(
            timeout_result.is_ok(),
            "handler must signal error immediately to waiters"
        );
        assert!(state.lock().unwrap().errored);
    }

    #[test]
    fn test_handler_parses_chart_data_without_cloning() {
        let state = Arc::new(Mutex::new(HistoricalState::new()));
        let factory = HistoricalDataHandlerFactory::new(Arc::clone(&state));
        let (cmd_tx, _cmd_rx) = tokio::sync::mpsc::channel(4);
        let handler = factory.create(cmd_tx);

        let chart_data_payload = serde_json::json!([
            "session_id",
            {
                "s1": {
                    "s": [
                        { "i": 100, "v": [100.0, 105.0, 99.0, 104.0, 1000.0] },
                        { "i": 101, "v": [104.0, 106.0, 103.0, 105.5, 1200.0] }
                    ]
                }
            }
        ]);

        let msg_slice = chart_data_payload.as_array().unwrap();
        handler.handle_events(TradingViewDataEvent::OnChartData, msg_slice);

        let guard = state.lock().unwrap();
        assert_eq!(guard.data.len(), 2);
        assert_eq!(guard.total_bars, 2);
        assert_eq!(guard.data[0].index, 100);
        assert_eq!(guard.data[1].index, 101);
    }
}
