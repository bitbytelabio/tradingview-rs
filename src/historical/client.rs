use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, instrument};

use crate::{
    DataServer, Error, Result,
    chart::ChartOptions,
    historical::{HistoricalRequest, HistoricalResult, state::HistoricalState},
    live::handler::Handler,
    live::websocket::WebSocketClient,
};

pub struct HistoricalClient<H: Handler> {
    auth_token: String,
    server: DataServer,
    _phantom: std::marker::PhantomData<H>,
}

impl<H: Handler> HistoricalClient<H> {
    pub fn new(auth_token: impl Into<String>, server: DataServer) -> Self {
        Self {
            auth_token: auth_token.into(),
            server,
            _phantom: std::marker::PhantomData,
        }
    }

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

        let mut state = if let Some(n) = request.num_bars {
            HistoricalState::with_capacity(n as usize)
        } else {
            HistoricalState::new()
        };

        self.run_session(&options, &mut state, request.timeout)
            .await?;

        let total_bars = state.total_bars;
        let data = state.finalize();
        let elapsed = started.elapsed();
        let symbol_info = state
            .symbol_info
            .take()
            .ok_or_else(|| Error::Internal("No symbol info received".into()))?;

        Ok(HistoricalResult {
            symbol_info,
            data,
            series_info: state.series_info.take(),
            total_bars_received: total_bars,
            replay_used: request.with_replay,
            elapsed,
        })
    }

    async fn run_session(
        &self,
        _options: &ChartOptions,
        state: &mut HistoricalState,
        timeout_dur: std::time::Duration,
    ) -> Result<()> {
        let ws = WebSocketClient::<H>::builder()
            .auth_token(&self.auth_token)
            .server(self.server)
            .handler(self.create_handler())
            .build()
            .await?;

        // spawn_reader_task takes self: Arc<Self>, so we clone the Arc.
        Arc::clone(&ws).spawn_reader_task();

        tokio::time::timeout(timeout_dur, self.wait_for_data(&ws, state))
            .await
            .map_err(|_| Error::Timeout("Historical data retrieval timed out".into()))?
    }

    fn create_handler(&self) -> H {
        unimplemented!("HistoricalClient requires a concrete Handler type parameter H")
    }

    async fn wait_for_data(
        &self,
        _ws: &WebSocketClient<H>,
        _state: &mut HistoricalState,
    ) -> Result<()> {
        Ok(())
    }
}
