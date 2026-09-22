//! Inline callback sink.
//!
//! The simplest possible sink: invokes a closure or function pointer for every
//! batch of events. Ideal for debugging, logging, and lightweight consumers.

use async_trait::async_trait;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

use super::EventSink;
use crate::Result;
use crate::events::MarketEvent;

/// A sink that invokes a user-supplied async function for each batch.
///
/// The callback receives a slice of [`MarketEvent`] and can process them
/// however it likes. This is the simplest way to wire up custom logic without
/// implementing [`EventSink`] directly.
pub struct CallbackSink<F> {
    callback: Arc<F>,
    name: String,
}

impl<F> CallbackSink<F> {
    /// Create a new callback sink.
    ///
    /// The name is used for logging and debugging.
    pub fn new(name: impl Into<String>, callback: F) -> Self {
        Self {
            callback: Arc::new(callback),
            name: name.into(),
        }
    }
}

// Fn variant — the async callback takes a slice of events.
#[async_trait]
impl<F, Fut> EventSink for CallbackSink<F>
where
    F: Fn(Vec<MarketEvent>) -> Fut + Send + Sync + 'static,
    Fut: std::future::Future<Output = Result<()>> + Send + 'static,
{
    async fn accept(&self, events: &[MarketEvent]) -> Result<()> {
        (self.callback)(events.to_vec()).await
    }

    fn name(&self) -> &str {
        &self.name
    }

    async fn shutdown(&self, _token: CancellationToken) -> Result<()> {
        Ok(())
    }
}

/// A blocking/sync variant that accepts a `FnMut` closure.
///
/// This spawns the blocking work onto `tokio::task::spawn_blocking` so it
/// doesn't starve the async runtime.
pub struct BlockingCallbackSink<F> {
    callback: Arc<std::sync::Mutex<F>>,
    name: String,
}

impl<F> BlockingCallbackSink<F>
where
    F: FnMut(&[MarketEvent]) -> Result<()> + Send + 'static,
{
    /// Create a new blocking callback sink.
    pub fn new(name: impl Into<String>, callback: F) -> Self {
        Self {
            callback: Arc::new(std::sync::Mutex::new(callback)),
            name: name.into(),
        }
    }
}

#[async_trait]
impl<F> EventSink for BlockingCallbackSink<F>
where
    F: FnMut(&[MarketEvent]) -> Result<()> + Send + 'static,
{
    async fn accept(&self, events: &[MarketEvent]) -> Result<()> {
        let owned = events.to_vec();
        let cb = Arc::clone(&self.callback);
        tokio::task::spawn_blocking(move || {
            let mut guard = cb.lock().map_err(|e| {
                crate::Error::Internal(ustr::ustr(&format!("callback lock poisoned: {e}")))
            })?;
            guard(&owned)
        })
        .await
        .map_err(|e| crate::Error::Internal(ustr::ustr(&format!("callback join: {e}"))))?
    }

    fn name(&self) -> &str {
        &self.name
    }

    async fn shutdown(&self, _token: CancellationToken) -> Result<()> {
        Ok(())
    }
}
