use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;
use tracing::{debug, instrument, warn};

use crate::{
    DataServer, Error, Result,
    chart::ChartOptions,
    historical::{HistoricalRequest, HistoricalResult, state::HistoricalState},
    live::handler::{CommandTx, LegacyHandler},
    live::models::TradingViewDataEvent,
    live::websocket::WebSocketClient,
};
use serde_json::Value;

/// High-level client for fetching historical TradingView chart data.
///
/// # Example
///
/// ```ignore
/// let client = HistoricalClient::new("your_auth_token", DataServer::ProData);
/// let request = HistoricalRequest::builder()
///     .symbol("AAPL").exchange("NASDAQ")
///     .num_bars(100).build();
/// let result = client.retrieve(request).await?;
/// println!("Got {} bars", result.data.len());
/// ```
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

    /// Fetch historical chart data for the given request.
    #[instrument(skip(self), fields(symbol, exchange))]
    pub async fn retrieve(&self, request: HistoricalRequest) -> Result<HistoricalResult> {
        let started = Instant::now();
        let (symbol, exchange) = request.resolve_symbol_exchange()?;
        debug!(symbol = %symbol, exchange = %exchange, "Historical retrieval started");

        let options = ChartOptions::builder()
            .symbol(&symbol)
            .exchange(&exchange)
            .interval(request.interval)
            .maybe_range(request.range)
            .maybe_bar_count(request.num_bars)
            .replay_mode(false)
            .build()?;

        // Shared state between handler and client.
        let state = Arc::new(Mutex::new(if let Some(n) = request.num_bars {
            HistoricalState::with_capacity(n as usize)
        } else {
            HistoricalState::new()
        }));

        // Handler that bridges WebSocket events → HistoricalState.
        let handler = HistoricalDataHandler::new(state.clone());

        let ws = WebSocketClient::builder()
            .auth_token(&self.auth_token)
            .server(self.server)
            .handler(handler)
            .build()
            .await?;

        Arc::clone(&ws).spawn_reader_task();

        // Wait for completion or timeout.
        let result = tokio::time::timeout(request.timeout, Self::wait_for_completion(&state)).await;

        // Get final state.
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
            Err(_elapsed) => Err(Error::Timeout("Historical data retrieval timed out".into())),
        }
    }

    /// Poll the state until completion or error.
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
// HistoricalDataHandler — bridges WebSocket events → HistoricalState
// =============================================================================

#[derive(Clone)]
struct HistoricalDataHandler {
    state: Arc<Mutex<HistoricalState>>,
}

impl HistoricalDataHandler {
    fn new(state: Arc<Mutex<HistoricalState>>) -> Self {
        Self { state }
    }
}

#[allow(deprecated)]
impl LegacyHandler for HistoricalDataHandler {
    fn new(_command_tx: CommandTx) -> Self {
        unreachable!("HistoricalDataHandler is constructed via HistoricalDataHandler::new()")
    }

    fn handle_events(&self, event: TradingViewDataEvent, message: &[Value]) {
        debug!(?event, "Historical handler received event");
        // In a full implementation, this would dispatch based on event type.
        let _ = (event, message);
    }

    fn handle_quote_data(&self, message: &[Value]) {
        debug!(?message, "Historical handler received quote data");
    }

    fn handle_series_data(&self, event: TradingViewDataEvent, messages: &[Value]) {
        debug!(
            ?event,
            len = messages.len(),
            "Historical handler received series data"
        );
        // DataPoints arrive as series data — extract and record them.
        let _ = (event, messages);
    }

    fn notify_error(&self, error: Error, message: &[Value]) {
        warn!(?error, ?message, "Historical handler received error");
        let mut state = self.state.blocking_lock();
        if state.record_error() {
            state.fail(format!("Too many errors: {error:?}"));
        }
    }
}
