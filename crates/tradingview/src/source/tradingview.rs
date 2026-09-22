//! TradingView data source adapter.
//!
//! Bridges the existing TradingView WebSocket client infrastructure to the
//! generic [`DataSource`] trait and [`MarketEvent`] pipeline.
//!
//! # Migration path
//!
//! The existing code in `live/` (WebSocket, handler, command runner) can be
//! incrementally adapted to implement this trait. The key change is replacing
//! the `Handler` trait's type-specific methods (`handle_quote_data`,
//! `handle_series_data`) with event normalization functions that produce
//! `MarketEvent` variants.

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use super::DataSource;
use crate::Result;
use crate::events::MarketEvent;

/// TradingView WebSocket data source.
///
/// Wraps the existing `WebSocketClient` to produce normalized events.
/// This is a forward-compatible design; full implementation requires
/// adapting the existing handler code to emit `MarketEvent` batches.
pub struct TradingViewSource {
    name: String,
}

impl TradingViewSource {
    /// Create a new TradingView data source.
    ///
    /// In the full implementation, this would accept configuration
    /// (auth_token, server, subscriptions) and initialize the
    /// `WebSocketClient` + `CommandRunner`.
    pub fn new() -> Self {
        Self {
            name: "tradingview".to_string(),
        }
    }
}

impl Default for TradingViewSource {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl DataSource for TradingViewSource {
    async fn run(
        &self,
        _sink: tokio::sync::mpsc::Sender<Vec<MarketEvent>>,
        _cancel: CancellationToken,
    ) -> Result<()> {
        // In the full implementation, this would:
        // 1. Create a WebSocketClient with the configured server/auth
        // 2. Create a CommandRunner
        // 3. Set up quote/chart sessions for the requested subscriptions
        // 4. Read messages from the WebSocket in a loop
        // 5. Normalize each raw message into MarketEvent variants
        // 6. Send batches through the sink channel
        // 7. Handle reconnection and error recovery
        // 8. Respect the CancellationToken for graceful shutdown

        // Placeholder: yield nothing
        let _ = self.name;
        Ok(())
    }

    fn name(&self) -> &str {
        &self.name
    }
}
