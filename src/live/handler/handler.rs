use serde_json::Value;
use tokio::sync::mpsc;

use crate::{
    Error,
    live::{handler::command::Command, models::TradingViewDataEvent},
};

/// Default capacity for the bounded command channel.
/// 256 commands of buffering before senders experience backpressure.
pub const DEFAULT_COMMAND_CHANNEL_CAPACITY: usize = 256;

/// Bounded sender for the command channel.
/// Replaces the old `UnboundedSender` — provides OOM protection via backpressure.
pub type CommandTx = mpsc::Sender<Command>;

/// Bounded receiver for the command channel.
pub type CommandRx = mpsc::Receiver<Command>;

/// Deprecated alias — use [`CommandTx`] (bounded) instead.
#[deprecated(since = "0.2.0", note = "Use CommandTx (bounded mpsc::Sender) instead")]
pub type UnboundedCommandTx = mpsc::UnboundedSender<Command>;

/// Deprecated alias — use [`CommandRx`] (bounded) instead.
#[deprecated(
    since = "0.2.0",
    note = "Use CommandRx (bounded mpsc::Receiver) instead"
)]
pub type UnboundedCommandRx = mpsc::UnboundedReceiver<Command>;

pub trait Handler: Clone + Send + Sync + 'static {
    fn new(command_tx: CommandTx) -> Self;
    fn handle_events(&self, event: TradingViewDataEvent, message: &[Value]);
    fn handle_quote_data(&self, message: &[Value]);
    fn handle_series_data(&self, event: TradingViewDataEvent, messages: &[Value]);
    fn notify_error(&self, error: Error, message: &[Value]);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    /// Verify that `CommandTx` is a bounded sender (not unbounded).
    #[test]
    fn test_command_tx_is_bounded() {
        let (tx, _rx) = mpsc::channel::<Command>(DEFAULT_COMMAND_CHANNEL_CAPACITY);
        let _command_tx: CommandTx = tx;
    }

    /// Verify that `CommandRx` is a bounded receiver.
    #[test]
    fn test_command_rx_is_bounded() {
        let (_tx, rx) = mpsc::channel::<Command>(DEFAULT_COMMAND_CHANNEL_CAPACITY);
        let _command_rx: CommandRx = rx;
    }

    /// Verify the default capacity is reasonable.
    #[test]
    fn test_default_capacity_is_reasonable() {
        assert!(DEFAULT_COMMAND_CHANNEL_CAPACITY >= 64);
        assert!(DEFAULT_COMMAND_CHANNEL_CAPACITY <= 4096);
    }

    /// Backward compat: old unbounded alias still works.
    #[test]
    #[allow(deprecated)]
    fn test_unbounded_alias_still_works() {
        let (tx, _rx) = mpsc::unbounded_channel::<Command>();
        let _old_tx: UnboundedCommandTx = tx;
    }

    /// Backward compat: old unbounded receiver alias.
    #[test]
    #[allow(deprecated)]
    fn test_unbounded_rx_alias_still_works() {
        let (_tx, rx) = mpsc::unbounded_channel::<Command>();
        let _old_rx: UnboundedCommandRx = rx;
    }

    /// Overload simulation: bounded channel applies backpressure.
    /// Sending more items than capacity should block (async) rather than
    /// silently growing unbounded memory.
    #[tokio::test]
    async fn test_bounded_channel_backpressure() {
        // Tiny capacity to trigger backpressure quickly
        let (tx, mut rx) = mpsc::channel::<u32>(4);

        // Fill the channel
        for i in 0..4 {
            tx.send(i).await.expect("send should succeed");
        }

        // Now the channel is full.  Verify we can drain and then send more.
        let consumer = tokio::spawn(async move {
            let mut drained = Vec::new();
            while let Some(val) = rx.recv().await {
                drained.push(val);
                if drained.len() == 8 {
                    break;
                }
            }
            drained
        });

        // Send 4 more — they'll be consumed as the consumer drains
        for i in 4..8 {
            tx.send(i)
                .await
                .expect("send should succeed after consumer drains");
        }

        drop(tx);
        let drained = consumer.await.unwrap();
        assert_eq!(drained, vec![0, 1, 2, 3, 4, 5, 6, 7]);
    }

    /// Overload simulation: try_send returns error when channel is full.
    #[tokio::test]
    async fn test_try_send_backpressure() {
        let (tx, mut _rx) = mpsc::channel::<u32>(2);

        // Fill the channel
        assert!(tx.try_send(1).is_ok());
        assert!(tx.try_send(2).is_ok());

        // Channel is full — try_send should fail
        assert!(tx.try_send(3).is_err());
    }
}
