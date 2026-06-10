//! Generic event-driven data loader.
//!
//! `DataLoader` is the central orchestrator that connects a [`DataSource`](crate::source::DataSource) to
//! one or more [`EventSink`](crate::sink::EventSink)s. It handles:
//!
//! - Source → fan-out task → per-sink tasks pipeline
//! - Bounded channels for backpressure
//! - Graceful shutdown via `CancellationToken`
//! - Error propagation
//!
//! # Example
//!
//! ```rust,ignore
//! use tradingview::loader::DataLoader;
//! use tradingview::sink::{ChannelSink, CallbackSink};
//! use tradingview::source::tradingview::TradingViewSource;
//!
//! # async fn example() -> tradingview::Result<()> {
//! let (channel_sink, mut rx) = ChannelSink::new(1024);
//!
//! let mut loader = DataLoader::builder()
//!     .source(TradingViewSource::new())
//!     .sink(channel_sink)
//!     .sink(CallbackSink::new("debug", |events| async move {
//!         for event in &events {
//!             tracing::debug!("{:?}", event);
//!         }
//!         Ok(())
//!     }))
//!     .build()?;
//!
//! loader.start().await?;
//! // ... events flow ...
//! loader.shutdown().await?;
//! # Ok(())
//! # }
//! ```

use tokio::{sync::mpsc, task::JoinSet};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use crate::{Result, events::MarketEvent, sink::EventSink, source::DataSource};

// ---------------------------------------------------------------------------
// Type aliases
// ---------------------------------------------------------------------------

/// A channel pair for dispatching event batches to a sink.
type EventChannel = (
    mpsc::Sender<Vec<MarketEvent>>,
    mpsc::Receiver<Vec<MarketEvent>>,
);

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for the data loader.
#[derive(Debug, Clone)]
pub struct LoaderConfig {
    /// Size of the bounded channel between source and fan-out task.
    /// Default: 4096.
    pub channel_capacity: usize,
    /// Maximum events per batch (for future batching logic).
    /// Default: 256.
    pub batch_size: usize,
    /// If `true`, the loader continues even if a sink rejects events.
    /// Default: `false`.
    pub continue_on_sink_error: bool,
}

impl Default for LoaderConfig {
    fn default() -> Self {
        Self {
            channel_capacity: 4096,
            batch_size: 256,
            continue_on_sink_error: false,
        }
    }
}

// ---------------------------------------------------------------------------
// DataLoader
// ---------------------------------------------------------------------------

/// Event-driven data loader — connects a source to multiple sinks.
///
/// Architecture:
///
/// ```text
/// Source task → source_tx ──→ [fan-out task] ──→ sink_tx_0 → sink task 0
///                                        ├───────→ sink_tx_1 → sink task 1
///                                        └───────→ sink_tx_N → sink task N
/// ```
///
/// The source produces `Vec<MarketEvent>` batches and sends them via a single
/// `mpsc` channel. A fan-out task reads from that channel and clones each
/// batch to every sink's individual channel. Each sink has its own task that
/// reads from its channel and calls `sink.accept()`.
pub struct DataLoader<S: DataSource = crate::source::tradingview::TradingViewSource> {
    source: Option<S>,
    sinks: Vec<Box<dyn EventSink>>,
    /// Per-sink (tx, rx) channel pairs.
    sink_channels: Vec<EventChannel>,
    config: LoaderConfig,
    cancel: CancellationToken,
    tasks: JoinSet<Result<()>>,
}

impl<S: DataSource> DataLoader<S> {
    /// Create a new builder with default configuration.
    pub fn builder() -> DataLoaderBuilder<S> {
        DataLoaderBuilder::new()
    }

    /// Start the loader: spawns source, fan-out, and per-sink tasks.
    ///
    /// This method consumes the source and sink channels; calling it twice
    /// returns an error.
    pub async fn start(&mut self) -> Result<()> {
        let source = self
            .source
            .take()
            .ok_or_else(|| crate::Error::Internal(ustr::ustr("loader already started")))?;

        if self.sinks.is_empty() {
            return Err(crate::Error::Internal(ustr::ustr(
                "no sinks registered — add at least one sink before starting",
            )));
        }

        let sink_count = self.sinks.len();
        info!(
            source = %source.name(),
            sink_count,
            channel_capacity = self.config.channel_capacity,
            "starting data loader",
        );

        // ---- Source → fan-out channel ----
        let (source_tx, source_rx) =
            mpsc::channel::<Vec<MarketEvent>>(self.config.channel_capacity);

        // ---- Spawn source task ----
        let cancel_src = self.cancel.clone();
        let source_name = source.name().to_string();
        self.tasks.spawn(async move {
            debug!(source = %source_name, "source task started");
            source.run(source_tx, cancel_src).await.inspect_err(|e| {
                error!(source = %source_name, error = %e, "source failed");
            })
        });

        // ---- Fan-out task ----
        let sink_txs: Vec<mpsc::Sender<Vec<MarketEvent>>> = self
            .sink_channels
            .iter()
            .map(|(tx, _)| tx.clone())
            .collect();
        let fan_cancel = self.cancel.clone();
        let fan_config = self.config.clone();
        self.tasks
            .spawn(async move { fan_out_task(source_rx, sink_txs, fan_config, fan_cancel).await });

        // ---- Per-sink tasks ----
        let sink_rxs: Vec<(usize, mpsc::Receiver<Vec<MarketEvent>>)> = self
            .sink_channels
            .drain(..)
            .enumerate()
            .map(|(i, (_, rx))| (i, rx))
            .collect();

        let mut sinks = std::mem::take(&mut self.sinks);

        for (idx, mut rx) in sink_rxs {
            // SAFETY: each sink is unique; we remove them in order.
            let sink = sinks.remove(0);
            let cancel_s = self.cancel.clone();
            let sink_name = sink.name().to_string();
            let cont_on_err = self.config.continue_on_sink_error;

            self.tasks.spawn(async move {
                sink_task(idx, sink, &mut rx, sink_name, cont_on_err, cancel_s).await
            });
        }

        Ok(())
    }

    /// Initiate graceful shutdown.
    ///
    /// Cancels all spawned tasks and waits for them to finish.
    pub async fn shutdown(&mut self) -> Result<()> {
        info!("initiating loader shutdown");
        self.cancel.cancel();

        while let Some(result) = self.tasks.join_next().await {
            match result {
                Ok(Ok(())) => debug!("task completed successfully"),
                Ok(Err(e)) => warn!(error = %e, "task completed with error"),
                Err(e) => warn!(error = %e, "task join error"),
            }
        }

        info!("loader shutdown complete");
        Ok(())
    }

    /// Returns a clone of the cancellation token for external monitoring.
    pub fn cancel_token(&self) -> CancellationToken {
        self.cancel.clone()
    }
}

// ---------------------------------------------------------------------------
// Background tasks
// ---------------------------------------------------------------------------

/// Fan-out: reads from `source_rx` and sends clones to every `sink_tx`.
async fn fan_out_task(
    mut source_rx: mpsc::Receiver<Vec<MarketEvent>>,
    sink_txs: Vec<mpsc::Sender<Vec<MarketEvent>>>,
    config: LoaderConfig,
    cancel: CancellationToken,
) -> Result<()> {
    loop {
        tokio::select! {
            biased;

            _ = cancel.cancelled() => {
                debug!("fan-out task cancelled");
                break;
            }

            result = source_rx.recv() => {
                match result {
                    Some(events) => {
                        for (i, tx) in sink_txs.iter().enumerate() {
                            if let Err(e) = tx.send(events.clone()).await {
                                if config.continue_on_sink_error {
                                    warn!(sink_index = i, error = %e, "sink channel dropped");
                                } else {
                                    return Err(crate::Error::Internal(ustr::ustr(
                                        &format!("sink channel {i} closed: {e}")
                                    )));
                                }
                            }
                        }
                    }
                    None => {
                        info!("source channel closed — fan-out exiting");
                        break;
                    }
                }
            }
        }
    }

    Ok(())
}

/// Single sink task: reads from `rx` and calls `sink.accept()`.
async fn sink_task(
    idx: usize,
    sink: Box<dyn EventSink>,
    rx: &mut mpsc::Receiver<Vec<MarketEvent>>,
    name: String,
    continue_on_error: bool,
    cancel: CancellationToken,
) -> Result<()> {
    debug!(sink_index = idx, sink = %name, "sink task started");

    loop {
        tokio::select! {
            biased;

            _ = cancel.cancelled() => {
                debug!(sink = %name, "sink task cancelled");
                // Give the sink a chance to flush
                let _ = sink.shutdown(cancel).await;
                break;
            }

            result = rx.recv() => {
                match result {
                    Some(events) => {
                        if let Err(e) = sink.accept(&events).await {
                            warn!(sink = %name, error = %e, "sink accept failed");
                            if !continue_on_error {
                                return Err(e);
                            }
                        }
                    }
                    None => {
                        debug!(sink = %name, "sink channel closed");
                        break;
                    }
                }
            }
        }
    }

    debug!(sink = %name, "sink task exiting");
    Ok(())
}

// ---------------------------------------------------------------------------
// Builder
// ---------------------------------------------------------------------------

/// Builder for [`DataLoader`].
///
/// # Example
///
/// ```rust,ignore
/// let loader = DataLoader::<TradingViewSource>::builder()
///     .source(tv_source)
///     .sink(channel_sink)
///     .channel_capacity(8192)
///     .build()?;
/// ```
#[must_use]
pub struct DataLoaderBuilder<S: DataSource> {
    source: Option<S>,
    sinks: Vec<Box<dyn EventSink>>,
    sink_channels: Vec<EventChannel>,
    config: LoaderConfig,
}

impl<S: DataSource> DataLoaderBuilder<S> {
    /// Create a new builder with defaults.
    pub fn new() -> Self {
        Self {
            source: None,
            sinks: Vec::new(),
            sink_channels: Vec::new(),
            config: LoaderConfig::default(),
        }
    }

    /// Set the data source (required).
    pub fn source(mut self, source: S) -> Self {
        self.source = Some(source);
        self
    }

    /// Register an event sink. You can call this multiple times to fan out
    /// to several sinks.
    pub fn sink(mut self, sink: impl EventSink) -> Self {
        let (tx, rx) = mpsc::channel::<Vec<MarketEvent>>(self.config.channel_capacity);
        self.sink_channels.push((tx, rx));
        let boxed: Box<dyn EventSink> = Box::new(sink);
        self.sinks.push(boxed);
        self
    }

    /// Override the default channel capacity (4096).
    pub fn channel_capacity(mut self, capacity: usize) -> Self {
        self.config.channel_capacity = capacity;
        self
    }

    /// If `true`, the loader keeps running even when a sink rejects events.
    pub fn continue_on_sink_error(mut self, val: bool) -> Self {
        self.config.continue_on_sink_error = val;
        self
    }

    /// Finish building the [`DataLoader`].
    ///
    /// # Errors
    ///
    /// - No source configured.
    /// - No sinks registered.
    pub fn build(self) -> Result<DataLoader<S>> {
        let source = self
            .source
            .ok_or_else(|| crate::Error::Internal(ustr::ustr("no data source configured")))?;

        if self.sinks.is_empty() {
            return Err(crate::Error::Internal(ustr::ustr(
                "at least one sink is required",
            )));
        }

        Ok(DataLoader {
            source: Some(source),
            sinks: self.sinks,
            sink_channels: self.sink_channels,
            config: self.config,
            cancel: CancellationToken::new(),
            tasks: JoinSet::new(),
        })
    }
}

impl<S: DataSource> Default for DataLoaderBuilder<S> {
    fn default() -> Self {
        Self::new()
    }
}
