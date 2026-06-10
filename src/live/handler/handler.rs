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

/// Deprecated alias — use [`CommandTx`] (bounded) instead.
#[deprecated(since = "0.2.0", note = "Use CommandTx (bounded mpsc::Sender) instead")]
pub type UnboundedCommandTx = mpsc::UnboundedSender<Command>;

/// Deprecated alias — use [`CommandRx`] (bounded) instead.
#[deprecated(
    since = "0.2.0",
    note = "Use CommandRx (bounded mpsc::Receiver) instead"
)]
pub type UnboundedCommandRx = mpsc::UnboundedReceiver<Command>;

// =============================================================================
// Handler trait — v2
// =============================================================================
//
// Migration guide:
//
//   v1 (deprecated)                    v2 (recommended)
//   ─────────────────────────────────  ──────────────────────────────────
//   trait Handler: Clone + Send + Sync trait Handler: Send + Sync + 'static
//   {                                   {
//       fn new(command_tx) -> Self;         // REMOVED — use HandlerFactory
//       fn handle_events(...);              fn handle_events(...);
//       fn handle_quote_data(...);          fn handle_quote_data(...);
//       fn handle_series_data(...);         fn handle_series_data(...);
//       fn notify_error(...);               fn notify_error(...);
//   }                                   }
//
//   // Step 1: Remove `Clone` from your impl (if you don't need it).
//   // Step 2: Remove `fn new()` from your impl — implement HandlerFactory
//   //         as a separate type or use the convenience blanket impl.
//   // Step 3: If you need shared ownership, use Arc<dyn Handler>.
//
//   // V1 code:
//   let handler = MyHandler::new(tx);
//
//   // V2 code:
//   let handler = MyHandlerFactory.create(tx);

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
// Backward compatibility — v1 bridge
// =============================================================================

/// Legacy version of the Handler trait — **deprecated**.
///
/// Kept for backward compatibility.  New code should implement [`Handler`]
/// and [`HandlerFactory`] instead.
#[deprecated(
    since = "0.2.0",
    note = "Implement Handler + HandlerFactory instead.  See migration guide in module docs."
)]
pub trait LegacyHandler: Clone + Send + Sync + 'static {
    fn new(command_tx: CommandTx) -> Self;
    fn handle_events(&self, event: TradingViewDataEvent, message: &[Value]);
    fn handle_quote_data(&self, message: &[Value]);
    fn handle_series_data(&self, event: TradingViewDataEvent, messages: &[Value]);
    fn notify_error(&self, error: Error, message: &[Value]);
}

// Blanket impl: anything implementing the old trait also implements the new
// ones.  This means all existing handlers continue to work without changes.
#[allow(deprecated)]
impl<T: LegacyHandler> Handler for T {
    fn handle_events(&self, event: TradingViewDataEvent, message: &[Value]) {
        LegacyHandler::handle_events(self, event, message);
    }
    fn handle_quote_data(&self, message: &[Value]) {
        LegacyHandler::handle_quote_data(self, message);
    }
    fn handle_series_data(&self, event: TradingViewDataEvent, messages: &[Value]) {
        LegacyHandler::handle_series_data(self, event, messages);
    }
    fn notify_error(&self, error: Error, message: &[Value]) {
        LegacyHandler::notify_error(self, error, message);
    }
}

#[allow(deprecated)]
impl<T: LegacyHandler> HandlerFactory for T {
    type Handler = T;
    fn create(&self, command_tx: CommandTx) -> Self::Handler {
        T::new(command_tx)
    }
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

    #[test]
    #[allow(deprecated)]
    fn test_unbounded_alias_still_works() {
        let (tx, _rx) = mpsc::unbounded_channel::<Command>();
        let _old_tx: UnboundedCommandTx = tx;
    }

    #[test]
    #[allow(deprecated)]
    fn test_unbounded_rx_alias_still_works() {
        let (_tx, rx) = mpsc::unbounded_channel::<Command>();
        let _old_rx: UnboundedCommandRx = rx;
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

    /// Verify that a type implementing the old `LegacyHandler` trait
    /// automatically satisfies the new `Handler` trait via the blanket impl.
    #[test]
    #[allow(deprecated)]
    fn test_legacy_handler_is_new_handler() {
        #[derive(Clone)]
        struct OldHandler;
        #[allow(deprecated)]
        impl LegacyHandler for OldHandler {
            fn new(_tx: CommandTx) -> Self {
                OldHandler
            }
            fn handle_events(&self, _e: TradingViewDataEvent, _m: &[Value]) {}
            fn handle_quote_data(&self, _m: &[Value]) {}
            fn handle_series_data(&self, _e: TradingViewDataEvent, _m: &[Value]) {}
            fn notify_error(&self, _e: Error, _m: &[Value]) {}
        }

        let (_tx, _rx) = mpsc::channel::<Command>(4);
        let handler = OldHandler::new(_tx);

        // Should compile — blanket impl bridges old → new
        fn accept_handler(_h: &impl Handler) {}
        accept_handler(&handler);

        // Also as factory
        fn accept_factory(_f: &impl HandlerFactory) {}
        accept_factory(&handler);
    }
}
