//! Event sink abstraction.
//!
//! Sinks are the consumer side of the event pipeline. The [`EventSink`] trait
//! defines a common interface for any downstream target. Built-in
//! implementations are provided for in-memory channels and inline callbacks.

pub mod callback;
pub mod channel;
pub mod kafka;

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use crate::Result;
use crate::events::MarketEvent;

/// The common interface for all event consumers.
///
/// Implementors receive batches of [`MarketEvent`] items. The trait is
/// `#[async_trait]` because sinks typically perform I/O (network send, disk
/// flush, etc.).
///
/// # Graceful shutdown
///
/// The `shutdown` method is called when the loader stops. Use it to flush
/// buffers or close connections.
#[async_trait]
pub trait EventSink: Send + Sync + 'static {
    /// Accept a batch of market events.
    ///
    /// Implementors should return an error only for non-recoverable failures.
    /// Transient errors (network timeout, etc.) should be retried internally.
    async fn accept(&self, events: &[MarketEvent]) -> Result<()>;

    /// Called when the loader is shutting down.
    ///
    /// Implementations should flush any buffered data and close connections.
    /// The returned future should not block indefinitely — it must respect the
    /// provided [`CancellationToken`].
    async fn shutdown(&self, _token: CancellationToken) -> Result<()> {
        Ok(())
    }

    /// A human-readable name for logging and debugging.
    fn name(&self) -> &str;
}
