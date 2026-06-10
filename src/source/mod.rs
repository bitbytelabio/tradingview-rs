//! Data source abstraction.
//!
//! Sources produce normalized [`MarketEvent`] batches and feed them into the
//! loader pipeline. The [`DataSource`] trait is the extension point for new
//! data providers (REST APIs, WebSocket feeds, CSV files, etc.).

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use crate::Result;
use crate::events::MarketEvent;

/// A provider of normalized market events.
///
/// A data source is responsible for:
/// 1. Connecting to the underlying data feed (WebSocket, REST, file).
/// 2. Fetching/streaming raw data.
/// 3. Normalizing raw data into [`MarketEvent`] variants.
/// 4. Sending events to the supplied `sink` channel.
///
/// The `run` method is the main execution loop. It runs until the feed is
/// exhausted, an unrecoverable error occurs, or the [`CancellationToken`] is
/// triggered.
#[async_trait]
pub trait DataSource: Send + Sync + 'static {
    /// Start streaming events.
    ///
    /// The implementation sends `Vec<MarketEvent>` batches to the provided
    /// async channel sender. The `cancel` token should be polled periodically
    /// to support graceful shutdown.
    async fn run(
        &self,
        sink: tokio::sync::mpsc::Sender<Vec<MarketEvent>>,
        cancel: CancellationToken,
    ) -> Result<()>;

    /// A human-readable name for logging and debugging.
    fn name(&self) -> &str;
}

/// Configuration for a data source subscription.
#[derive(Debug, Clone)]
pub struct Subscription {
    /// The instrument to subscribe to (e.g. `"BINANCE:BTCUSDT"`, `"AAPL"`).
    pub symbol: String,
    /// What kind of data to request.
    pub data_kind: crate::events::DataKind,
    /// Optional bar interval for candle subscriptions.
    pub interval: Option<crate::models::Interval>,
    /// Optional number of bars to fetch (historical only).
    pub bar_count: Option<u64>,
}

impl Subscription {
    /// Create a new candle subscription for a symbol.
    pub fn candle(symbol: impl Into<String>, interval: crate::models::Interval) -> Self {
        Self {
            symbol: symbol.into(),
            data_kind: crate::events::DataKind::Candle,
            interval: Some(interval),
            bar_count: None,
        }
    }

    /// Create a new quote subscription for a symbol.
    pub fn quote(symbol: impl Into<String>) -> Self {
        Self {
            symbol: symbol.into(),
            data_kind: crate::events::DataKind::Quote,
            interval: None,
            bar_count: None,
        }
    }

    /// Create a new economic data subscription.
    pub fn economic(indicator_id: impl Into<String>) -> Self {
        Self {
            symbol: indicator_id.into(),
            data_kind: crate::events::DataKind::Economic,
            interval: None,
            bar_count: None,
        }
    }
}

/// TradingView WebSocket data source adapter.
///
/// Wraps the existing [`WebSocketClient`] and `CommandRunner` infrastructure
/// to produce normalized [`MarketEvent`] batches.
///
/// Note: This is a forward-looking design. The full implementation that bridges
/// the existing WebSocket handler to the new event pipeline requires the
/// existing handler code to be adapted. This module provides the trait
/// interface and a skeleton that can be completed incrementally.
pub mod tradingview;
