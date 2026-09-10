//! High-level client for fetching TradingView study and fundamental data.

use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde::Deserialize;
use serde_json::Value;
use tracing::{debug, error, instrument, trace, warn};

use crate::Result;
use crate::chart::{DataPoint, SymbolInfo};
use crate::error::{Error, TradingViewError};
use crate::live::handler::command::Command;
use crate::live::handler::{CommandTx, Handler, HandlerFactory};
use crate::live::models::{DataServer, TradingViewDataEvent};
use crate::live::websocket::WebSocketClient;
use crate::study::request::StudyRequest;
use crate::study::result::StudyResult;
use crate::study::state::StudyState;
use crate::utils::symbol_init;

/// High-level client for fetching TradingView study and fundamental data.
pub struct StudyClient {
    pub(crate) auth_token: String,
    pub(crate) server: DataServer,
}

impl StudyClient {
    /// Creates a new [`StudyClient`] with the specified auth token and data server.
    pub fn new(auth_token: impl Into<String>, server: DataServer) -> Self {
        Self {
            auth_token: auth_token.into(),
            server,
        }
    }

    /// Retrieves study or fundamental data according to the given request.
    #[instrument(skip(self), fields(symbol, exchange))]
    pub async fn retrieve(&self, request: StudyRequest) -> Result<StudyResult> {
        let started = Instant::now();
        let (symbol, exchange) = request.resolve_symbol_exchange()?;
        debug!(symbol = %symbol, exchange = %exchange, "study retrieval started");

        let study_id = request
            .study_id
            .clone()
            .unwrap_or_else(|| format!("st_{}", crate::utils::gen_id()));
        let study_sub_id = "st1".to_string();

        let state = Arc::new(Mutex::new(StudyState::with_capacity_and_notify(
            request.base_bar_count as usize,
            Arc::new(tokio::sync::Notify::new()),
        )));

        let (cmd_tx, _cmd_rx) = tokio::sync::mpsc::channel::<Command>(16);
        let factory = StudyDataHandlerFactory::new(Arc::clone(&state), &study_id);
        let handler = factory.create(cmd_tx);

        let ws = WebSocketClient::builder()
            .auth_token(&self.auth_token)
            .server(self.server)
            .handler(handler)
            .build()
            .await?;

        // ── Protocol sequence ──────────────────────────────────────────
        // TradingView study retrieval protocol:
        //   1. chart_create_session → create chart session
        //   2. resolve_symbol       → resolve instrument and receive SymbolInfo
        //   3. create_series        → create base price series
        //   4. create_study         → attach study to chart series (sds_1)
        //   5. wait for matching study_completed or error
        let instrument = format!("{exchange}:{symbol}");
        let chart_session = format!("cs_{}", crate::utils::gen_id());
        let symbol_series_id = format!("sds_sym_{}", crate::utils::gen_id());
        let series_identifier = "sds_1".to_string();
        let series_id = "s1".to_string();

        // 1. Create chart session.
        ws.create_chart_session(&chart_session).await?;
        debug!(session = %chart_session, "chart session created");

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
        debug!(instrument = %instrument, "symbol resolution requested");

        // 3. Create base data series.
        let create_series_args = crate::historical::client::build_create_series_args(
            &chart_session,
            &series_identifier,
            &series_id,
            &symbol_series_id,
            request.interval,
            Some(request.base_bar_count),
            None,
        );
        ws.send("create_series", &create_series_args).await?;
        debug!(
            interval = ?request.interval,
            bars = request.base_bar_count,
            "base series created"
        );

        // 4. Create study attached to base series reference (sds_1).
        ws.create_study()
            .chart_session(&chart_session)
            .study_ids(&[&study_id, &study_sub_id])
            .chart_series_id(&series_identifier)
            .study(request.study)
            .call()
            .await?;
        debug!(study_id = %study_id, "study created");

        Arc::clone(&ws).spawn_reader_task();

        let result = tokio::time::timeout(request.timeout, Self::wait_for_completion(&state)).await;

        let mut state_guard = state.lock().unwrap();
        let total_points = state_guard.total_points;
        let data = state_guard.finalize();
        let elapsed = started.elapsed();

        match result {
            Ok(_) => {
                if state_guard.errored {
                    let msg = state_guard
                        .error_message
                        .take()
                        .unwrap_or_else(|| "study data retrieval failed".to_string());
                    return Err(Error::Internal(msg.into()));
                }
                let symbol_info = state_guard
                    .symbol_info
                    .take()
                    .ok_or_else(|| Error::Internal("no symbol info received".into()))?;
                Ok(StudyResult {
                    symbol_info,
                    data,
                    study_id,
                    total_points_received: total_points,
                    elapsed,
                })
            }
            Err(_) => Err(Error::Timeout("study data retrieval timed out".into())),
        }
    }

    pub(crate) async fn wait_for_completion(state: &Arc<Mutex<StudyState>>) {
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
                if guard.is_done() {
                    break;
                }
            }

            notified.await;
        }
    }
}

// =============================================================================
// StudyDataHandler
// =============================================================================

/// Event handler that accumulates study data points into shared [`StudyState`].
#[derive(Clone)]
pub(crate) struct StudyDataHandler {
    state: Arc<Mutex<StudyState>>,
    study_id: String,
    #[allow(dead_code)]
    cmd_tx: CommandTx,
}

impl Handler for StudyDataHandler {
    fn handle_events(&self, event: TradingViewDataEvent, message: &[Value]) {
        match event {
            TradingViewDataEvent::OnSymbolResolved => {
                // resolve_symbol response: [session, symbol_series_id, SymbolInfo]
                if let Some(sym_info) = message.get(2)
                    && let Ok(info) = SymbolInfo::deserialize(sym_info)
                {
                    debug!(name = %info.name, "symbol resolved");
                    self.state.lock().unwrap().record_symbol_info(info);
                }
            }
            TradingViewDataEvent::OnChartData | TradingViewDataEvent::OnChartDataUpdate => {
                // timescale_update or du message:
                // message[0]: chart_session
                // message[1]: object with study_id or series_id keys
                if message.len() < 2 {
                    return;
                }
                if let Some(obj) = message[1].as_object()
                    && let Some(study_val) = obj.get(&self.study_id)
                    && let Some(st_arr) = study_val.get("st").and_then(|v| v.as_array())
                {
                    let mut points = Vec::with_capacity(st_arr.len());
                    for v in st_arr {
                        if let Ok(point) = DataPoint::deserialize(v) {
                            points.push(point);
                        }
                    }
                    if !points.is_empty() {
                        let count = points.len();
                        let mut state = self.state.lock().unwrap();
                        state.record_points(points, count);
                    }
                }
            }
            TradingViewDataEvent::OnStudyCompleted => {
                // study_completed message: [chart_session, study_id, study_sub_id]
                let completed_id = message.get(1).and_then(|v| v.as_str());
                if completed_id == Some(&self.study_id) {
                    debug!(study_id = %self.study_id, "study completed");
                    self.state.lock().unwrap().complete();
                } else {
                    trace!(
                        completed = ?completed_id,
                        expected = %self.study_id,
                        "ignoring study_completed for unrelated study"
                    );
                }
            }
            TradingViewDataEvent::OnError(tv_error) => {
                // If the error is a study_error with an explicit study_id not matching ours, ignore it.
                if tv_error == TradingViewError::StudyError
                    && let Some(err_study_id) = message.get(1).and_then(|v| v.as_str())
                    && err_study_id != self.study_id
                {
                    warn!(
                        err_study_id = %err_study_id,
                        expected = %self.study_id,
                        "ignoring study_error for unrelated study"
                    );
                    return;
                }

                error!(?tv_error, "tradingview study/protocol error");
                let err_details = message
                    .iter()
                    .filter_map(|v| v.as_str())
                    .collect::<Vec<_>>()
                    .join(" ");
                let msg = if err_details.is_empty() {
                    format!("tradingview error: {tv_error:?}")
                } else {
                    format!("tradingview error: {tv_error:?}: {err_details}")
                };
                let mut state = self.state.lock().unwrap();
                state.fail(msg);
            }
            _ => {}
        }
    }

    fn handle_quote_data(&self, _message: &[Value]) {}
    fn handle_series_data(&self, _event: TradingViewDataEvent, _messages: &[Value]) {}

    fn notify_error(&self, error: Error, _message: &[Value]) {
        warn!(?error, "study handler error");
        let mut state = self.state.lock().unwrap();
        if state.record_error() {
            state.fail(format!("too many errors: {error:?}"));
        }
    }
}

// =============================================================================
// StudyDataHandlerFactory
// =============================================================================

/// Factory for creating [`StudyDataHandler`] instances that share a common [`StudyState`].
pub(crate) struct StudyDataHandlerFactory {
    state: Arc<Mutex<StudyState>>,
    study_id: String,
}

impl StudyDataHandlerFactory {
    /// Create a new factory wrapping the given shared state and study ID.
    pub fn new(state: Arc<Mutex<StudyState>>, study_id: impl Into<String>) -> Self {
        Self {
            state,
            study_id: study_id.into(),
        }
    }
}

impl HandlerFactory for StudyDataHandlerFactory {
    type Handler = StudyDataHandler;

    fn create(&self, command_tx: CommandTx) -> Self::Handler {
        StudyDataHandler {
            state: Arc::clone(&self.state),
            study_id: self.study_id.clone(),
            cmd_tx: command_tx,
        }
    }
}

// =============================================================================
// Unit Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn setup_handler(study_id: &str) -> (StudyDataHandler, Arc<Mutex<StudyState>>) {
        let state = Arc::new(Mutex::new(StudyState::new()));
        let (cmd_tx, _cmd_rx) = tokio::sync::mpsc::channel(16);
        let handler = StudyDataHandler {
            state: Arc::clone(&state),
            study_id: study_id.to_string(),
            cmd_tx,
        };
        (handler, state)
    }

    #[test]
    fn test_matching_study_id_routing() {
        let (handler, state) = setup_handler("st_target");

        // Payload with target study and points
        let payload = json!([
            "cs_test",
            {
                "st_target": {
                    "st": [
                        {"i": -10, "v": [1600000000.0, 100.5, 200.5]},
                        {"i": -9, "v": [1600086400.0, 105.0, 210.0]}
                    ]
                }
            }
        ]);

        handler.handle_events(
            TradingViewDataEvent::OnChartData,
            payload.as_array().unwrap(),
        );

        let guard = state.lock().unwrap();
        assert_eq!(guard.data.len(), 2);
        assert_eq!(guard.data[0].index, -10);
        assert_eq!(guard.data[0].value, vec![1600000000.0, 100.5, 200.5]);
        assert_eq!(guard.data[1].index, -9);
        assert_eq!(guard.data[1].value, vec![1600086400.0, 105.0, 210.0]);
        assert_eq!(guard.total_points, 2);
    }

    #[test]
    fn test_ignoring_unrelated_studies() {
        let (handler, state) = setup_handler("st_target");

        // Payload with only unrelated study and base series
        let payload = json!([
            "cs_test",
            {
                "st_unrelated": {
                    "st": [
                        {"i": 0, "v": [1600000000.0, 999.0]}
                    ]
                },
                "sds_1": {
                    "s": [
                        {"i": 0, "v": [1600000000.0, 10.0, 20.0, 5.0, 15.0, 1000.0]}
                    ]
                }
            }
        ]);

        handler.handle_events(
            TradingViewDataEvent::OnChartDataUpdate,
            payload.as_array().unwrap(),
        );

        let guard = state.lock().unwrap();
        assert!(guard.data.is_empty());
        assert_eq!(guard.total_points, 0);
    }

    #[test]
    fn test_mixed_payload_extracts_only_target() {
        let (handler, state) = setup_handler("st_target");

        let payload = json!([
            "cs_test",
            {
                "st_unrelated": {
                    "st": [{"i": 1, "v": [100.0, 999.0]}]
                },
                "st_target": {
                    "st": [{"i": 2, "v": [200.0, 888.0]}]
                }
            }
        ]);

        handler.handle_events(
            TradingViewDataEvent::OnChartData,
            payload.as_array().unwrap(),
        );

        let guard = state.lock().unwrap();
        assert_eq!(guard.data.len(), 1);
        assert_eq!(guard.data[0].index, 2);
        assert_eq!(guard.data[0].value, vec![200.0, 888.0]);
    }

    #[test]
    fn test_completion_filtering() {
        let (handler, state) = setup_handler("st_target");

        // Unrelated completion
        let unrelated = json!(["cs_test", "st_other", "s1_st1"]);
        handler.handle_events(
            TradingViewDataEvent::OnStudyCompleted,
            unrelated.as_array().unwrap(),
        );
        assert!(!state.lock().unwrap().completed);

        // Matching completion
        let matching = json!(["cs_test", "st_target", "s1_st1"]);
        handler.handle_events(
            TradingViewDataEvent::OnStudyCompleted,
            matching.as_array().unwrap(),
        );
        assert!(state.lock().unwrap().completed);
    }

    #[test]
    fn test_study_error_filtering() {
        let (handler, state) = setup_handler("st_target");

        // Unrelated study error should be ignored
        let unrelated_err = json!(["cs_test", "st_other", "study initialization failed"]);
        handler.handle_events(
            TradingViewDataEvent::OnError(TradingViewError::StudyError),
            unrelated_err.as_array().unwrap(),
        );
        assert!(!state.lock().unwrap().errored);

        // Matching study error should fail state
        let matching_err = json!(["cs_test", "st_target", "invalid indicator parameters"]);
        handler.handle_events(
            TradingViewDataEvent::OnError(TradingViewError::StudyError),
            matching_err.as_array().unwrap(),
        );
        let guard = state.lock().unwrap();
        assert!(guard.errored);
        assert!(
            guard
                .error_message
                .as_ref()
                .unwrap()
                .contains("invalid indicator parameters")
        );
    }

    #[test]
    fn test_protocol_error_triggers_failure() {
        let (handler, state) = setup_handler("st_target");

        let protocol_err = json!(["cs_test", "critical error occurred"]);
        handler.handle_events(
            TradingViewDataEvent::OnError(TradingViewError::CriticalError),
            protocol_err.as_array().unwrap(),
        );
        let guard = state.lock().unwrap();
        assert!(guard.errored);
        assert!(guard.error_message.is_some());
    }

    #[test]
    fn test_sort_and_dedup_points_replaces_with_latest_point() {
        let mut state = StudyState::new();

        // Points arriving in chronological sequence:
        // 1. Point at ts=1600020000, idx=10 with initial value 10.0
        // 2. Point at ts=1600010000, idx=5 with value 100.0
        // 3. Update to ts=1600020000, idx=10 with NEW value -42.5 (should overwrite 10.0)
        // 4. Point at ts=1600000000, idx=1 with value 0.0
        let p1 = DataPoint {
            index: 10,
            value: vec![1600020000.0, 10.0],
        };
        let p2 = DataPoint {
            index: 5,
            value: vec![1600010000.0, 100.0],
        };
        let p1_update = DataPoint {
            index: 10,
            value: vec![1600020000.0, -42.5, 1e100],
        };
        let p0 = DataPoint {
            index: 1,
            value: vec![1600000000.0, 0.0],
        };

        state.record_points(vec![p1, p2, p1_update, p0], 4);
        let finalized = state.finalize();

        // Must have 3 points ordered ascending by timestamp
        assert_eq!(finalized.len(), 3);
        assert_eq!(finalized[0].value[0] as i64, 1600000000);
        assert_eq!(finalized[1].value[0] as i64, 1600010000);
        assert_eq!(finalized[2].value[0] as i64, 1600020000);
        // Latest point for index 10 must replace earlier value:
        assert_eq!(finalized[2].value[1], -42.5);
        assert_eq!(finalized[2].value[2], 1e100);
    }

    #[test]
    fn test_captured_time_scale_update_fixture() {
        let fixture_str = include_str!("../../tests/data/time_scale_update.json");
        let messages: Vec<Value> = serde_json::from_str(fixture_str).unwrap();

        // Track "st3" which appears in the fixture
        let (handler, state) = setup_handler("st3");

        for msg in &messages {
            let m = msg["m"].as_str().unwrap();
            let p = msg["p"].as_array().unwrap();
            let event = TradingViewDataEvent::from(m.to_string());
            handler.handle_events(event, p);
        }

        let mut guard = state.lock().unwrap();
        assert!(guard.completed, "st3 should have received study_completed");
        assert!(!guard.errored);
        assert!(!guard.data.is_empty(), "st3 should have captured points");

        let points = guard.finalize();
        // Points must be sorted ascending by timestamp
        for window in points.windows(2) {
            let ts_a = window[0].value[0] as i64;
            let ts_b = window[1].value[0] as i64;
            assert!(
                ts_a <= ts_b,
                "points must be ordered ascending: {ts_a} > {ts_b}"
            );
        }
    }

    #[test]
    fn test_captured_fixture_ignores_unrelated() {
        let fixture_str = include_str!("../../tests/data/time_scale_update.json");
        let messages: Vec<Value> = serde_json::from_str(fixture_str).unwrap();

        // Track a study ID not present in the fixture
        let (handler, state) = setup_handler("st_nonexistent");

        for msg in &messages {
            let m = msg["m"].as_str().unwrap();
            let p = msg["p"].as_array().unwrap();
            let event = TradingViewDataEvent::from(m.to_string());
            handler.handle_events(event, p);
        }

        let guard = state.lock().unwrap();
        assert!(!guard.completed);
        assert!(guard.data.is_empty());
    }
}
