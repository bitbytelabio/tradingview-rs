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
pub type CommandTx = mpsc::Sender<Command>;

/// Bounded receiver for the command channel.
pub type CommandRx = mpsc::Receiver<Command>;

// =============================================================================
// Handler trait
// =============================================================================

/// Core event handler trait — object-safe, supports `Arc<dyn Handler>`.
///
/// Unlike v1, this trait does **not** require `Clone` or a `new()`
/// constructor.  Use [`HandlerFactory`] for construction and `Arc<dyn
/// Handler>` when shared ownership is needed.
pub trait Handler: Send + Sync + 'static {
    /// Called when a TradingView data event is received.
    fn handle_events(&self, event: TradingViewDataEvent, message: &[Value]);

    /// Called when quote data is received (e.g., price updates).
    fn handle_quote_data(&self, message: &[Value]);

    /// Called when series/historical data is received.
    fn handle_series_data(&self, event: TradingViewDataEvent, messages: &[Value]);

    /// Called when an error occurs in the WebSocket or command pipeline.
    fn notify_error(&self, error: Error, message: &[Value]);
}

/// Factory trait for constructing [`Handler`] implementations.
///
/// Separating construction from the handler trait enables dependency
/// injection and cleaner initialization patterns.
pub trait HandlerFactory: Send + Sync + 'static {
    /// The concrete handler type produced by this factory.
    type Handler: Handler;

    /// Create a new handler instance, passing the command sender so the
    /// handler can issue commands back to the WebSocket client.
    fn create(&self, command_tx: CommandTx) -> Self::Handler;
}



// =============================================================================
// Tests
// =============================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    // ------------------------------------------------------------------
    // Channel tests (existing)
    // ------------------------------------------------------------------

    #[test]
    fn test_command_tx_is_bounded() {
        let (tx, _rx) = mpsc::channel::<Command>(DEFAULT_COMMAND_CHANNEL_CAPACITY);
        let _command_tx: CommandTx = tx;
    }

    #[test]
    fn test_command_rx_is_bounded() {
        let (_tx, rx) = mpsc::channel::<Command>(DEFAULT_COMMAND_CHANNEL_CAPACITY);
        let _command_rx: CommandRx = rx;
    }

    #[test]
    fn test_default_capacity_is_reasonable() {
        assert!(DEFAULT_COMMAND_CHANNEL_CAPACITY >= 64);
        assert!(DEFAULT_COMMAND_CHANNEL_CAPACITY <= 4096);
    }

    #[tokio::test]
    async fn test_bounded_channel_backpressure() {
        let (tx, mut rx) = mpsc::channel::<u32>(4);
        for i in 0..4 {
            tx.send(i).await.expect("send should succeed");
        }
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
        for i in 4..8 {
            tx.send(i).await.expect("send after drain");
        }
        drop(tx);
        let drained = consumer.await.unwrap();
        assert_eq!(drained, vec![0, 1, 2, 3, 4, 5, 6, 7]);
    }

    #[tokio::test]
    async fn test_try_send_backpressure() {
        let (tx, mut _rx) = mpsc::channel::<u32>(2);
        assert!(tx.try_send(1).is_ok());
        assert!(tx.try_send(2).is_ok());
        assert!(tx.try_send(3).is_err());
    }

    // ------------------------------------------------------------------
    // Handler v2 tests
    // ------------------------------------------------------------------

    /// A minimal handler implementation using the new v2 traits.
    struct TestHandler {
        events: std::sync::Mutex<Vec<String>>,
    }

    impl Handler for TestHandler {
        fn handle_events(&self, _event: TradingViewDataEvent, message: &[Value]) {
            self.events
                .lock()
                .unwrap()
                .push(format!("event: {:?}", message));
        }
        fn handle_quote_data(&self, message: &[Value]) {
            self.events
                .lock()
                .unwrap()
                .push(format!("quote: {:?}", message));
        }
        fn handle_series_data(&self, _event: TradingViewDataEvent, messages: &[Value]) {
            self.events
                .lock()
                .unwrap()
                .push(format!("series: {:?}", messages));
        }
        fn notify_error(&self, _error: Error, message: &[Value]) {
            self.events
                .lock()
                .unwrap()
                .push(format!("error: {:?}", message));
        }
    }

    struct TestHandlerFactory;
    impl HandlerFactory for TestHandlerFactory {
        type Handler = TestHandler;
        fn create(&self, _command_tx: CommandTx) -> Self::Handler {
            TestHandler {
                events: std::sync::Mutex::new(Vec::new()),
            }
        }
    }

    #[test]
    fn test_new_handler_compiles_and_works() {
        let (_tx, _rx) = mpsc::channel::<Command>(4);
        let factory = TestHandlerFactory;
        let handler = factory.create(_tx);
        handler.handle_events(
            TradingViewDataEvent::OnChartData,
            &[serde_json::json!({"test": true})],
        );
        let events = handler.events.lock().unwrap();
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn test_handler_is_object_safe() {
        let (_tx, _rx) = mpsc::channel::<Command>(4);
        let factory = TestHandlerFactory;
        let handler = factory.create(_tx);
        // Verify we can use Arc<dyn Handler>
        let _arc: std::sync::Arc<dyn Handler> = std::sync::Arc::new(handler);
    }

}
