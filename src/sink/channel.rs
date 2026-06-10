//! In-memory channel sink.
//!
//! Wraps a `tokio::sync::mpsc::Sender` so that events are forwarded to another
//! task within the same process. This is the simplest sink and is useful for
//! integration tests and single-process architectures.

use async_trait::async_trait;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::EventSink;
use crate::Result;
use crate::events::MarketEvent;

/// A sink that sends events over a bounded `mpsc` channel.
///
/// # Backpressure
///
/// When the channel buffer is full, `accept()` will wait (async-block) until
/// the receiver picks up items. This provides natural backpressure.
pub struct ChannelSink {
    tx: mpsc::Sender<Vec<MarketEvent>>,
    name: String,
}

impl ChannelSink {
    /// Create a new channel sink with a buffer capacity of `bound`.
    ///
    /// Returns the sink and the receiver half. The receiver must be consumed
    /// by another task; otherwise the sink will deadlock.
    pub fn new(bound: usize) -> (Self, mpsc::Receiver<Vec<MarketEvent>>) {
        let (tx, rx) = mpsc::channel(bound);
        (
            Self {
                tx,
                name: "channel".to_string(),
            },
            rx,
        )
    }

    /// Create a channel sink from an existing sender.
    pub fn from_sender(tx: mpsc::Sender<Vec<MarketEvent>>) -> Self {
        Self {
            tx,
            name: "channel".to_string(),
        }
    }
}

#[async_trait]
impl EventSink for ChannelSink {
    async fn accept(&self, events: &[MarketEvent]) -> Result<()> {
        self.tx
            .send(events.to_vec())
            .await
            .map_err(|_| crate::Error::Internal(ustr::ustr("channel sink receiver dropped")))?;
        Ok(())
    }

    async fn shutdown(&self, _token: CancellationToken) -> Result<()> {
        // Channel is implicitly closed when the sender is dropped.
        // Nothing to do here.
        Ok(())
    }

    fn name(&self) -> &str {
        &self.name
    }
}
