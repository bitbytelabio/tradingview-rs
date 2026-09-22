//! Tests for the event-driven data loader (integration tests).

use async_trait::async_trait;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tradingview::Result;
use tradingview::events::{CandleData, MarketEvent};
use tradingview::loader::DataLoader;
use tradingview::sink::{callback::CallbackSink, channel::ChannelSink};
use tradingview::source::DataSource;

// ------------------------------------------------------------------
// Mock source for testing
// ------------------------------------------------------------------

/// A mock source that emits a fixed set of events.
#[derive(Clone)]
struct MockSource {
    events: Vec<Vec<MarketEvent>>,
    name: String,
    /// If set, emit this error instead of events.
    error: Option<String>,
}

impl MockSource {
    fn new(events: Vec<Vec<MarketEvent>>) -> Self {
        Self {
            events,
            name: "mock".to_string(),
            error: None,
        }
    }

    #[allow(dead_code)]
    fn with_error(mut self, error: &str) -> Self {
        self.error = Some(error.to_string());
        self
    }
}

#[async_trait]
impl DataSource for MockSource {
    async fn run(
        &self,
        sink: mpsc::Sender<Vec<MarketEvent>>,
        cancel: CancellationToken,
    ) -> Result<()> {
        if let Some(ref err) = self.error {
            return Err(tradingview::Error::Internal(ustr::ustr(err)));
        }

        for batch in &self.events {
            if cancel.is_cancelled() {
                break;
            }
            if sink.send(batch.clone()).await.is_err() {
                break;
            }
            // Small yield to let sinks process
            tokio::task::yield_now().await;
        }
        Ok(())
    }

    fn name(&self) -> &str {
        &self.name
    }
}

// ------------------------------------------------------------------
// Helper: create a candle event
// ------------------------------------------------------------------

fn make_candle(ts: i64, symbol: &str, o: f64, h: f64, l: f64, c: f64, v: f64) -> MarketEvent {
    MarketEvent::Candle(CandleData::new(ts, symbol, "1D", o, h, l, c, v))
}

// ------------------------------------------------------------------
// Tests
// ------------------------------------------------------------------

#[tokio::test]
async fn test_channel_sink_receives_events() {
    let (channel_sink, mut rx) = ChannelSink::new(16);
    let events = vec![make_candle(
        1000,
        "AAPL",
        150.0,
        155.0,
        149.0,
        153.0,
        1_000_000.0,
    )];

    let source = MockSource::new(vec![events.clone()]);
    let mut loader = DataLoader::builder()
        .source(source)
        .sink(channel_sink)
        .build()
        .expect("build failed");

    loader.start().await.expect("start failed");

    // Receive from the channel
    let received = rx.recv().await.expect("no events received");
    assert_eq!(received.len(), 1);

    if let MarketEvent::Candle(c) = &received[0] {
        assert_eq!(c.symbol.as_str(), "AAPL");
        assert!((c.open - 150.0).abs() < f64::EPSILON);
        assert!((c.close - 153.0).abs() < f64::EPSILON);
    } else {
        panic!("expected Candle event");
    }

    loader.shutdown().await.expect("shutdown failed");
}

#[tokio::test]
async fn test_callback_sink_receives_events() {
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = Arc::clone(&counter);

    let callback = CallbackSink::new("test-cb", move |events: Vec<MarketEvent>| {
        let cnt = Arc::clone(&counter_clone);
        async move {
            cnt.fetch_add(events.len(), Ordering::SeqCst);
            Ok(())
        }
    });

    let events = vec![
        make_candle(1000, "MSFT", 300.0, 305.0, 299.0, 304.0, 500_000.0),
        make_candle(2000, "MSFT", 304.0, 310.0, 303.0, 308.0, 600_000.0),
    ];

    let source = MockSource::new(vec![events]);
    let mut loader = DataLoader::builder()
        .source(source)
        .sink(callback)
        .build()
        .expect("build failed");

    loader.start().await.expect("start failed");

    // Let the events propagate through the pipeline
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    loader.shutdown().await.expect("shutdown failed");

    let count = counter.load(Ordering::SeqCst);
    assert_eq!(count, 2, "expected 2 events via callback, got {count}");
}

#[tokio::test]
async fn test_multiple_sinks() {
    let (sink1, mut rx1) = ChannelSink::new(16);
    let (sink2, mut rx2) = ChannelSink::new(16);

    let events = vec![make_candle(
        3000,
        "TSLA",
        200.0,
        205.0,
        195.0,
        202.0,
        2_000_000.0,
    )];
    let source = MockSource::new(vec![events]);

    let mut loader = DataLoader::builder()
        .source(source)
        .sink(sink1)
        .sink(sink2)
        .build()
        .expect("build failed");

    loader.start().await.expect("start failed");

    let r1 = rx1.recv().await.expect("sink1: no events");
    let r2 = rx2.recv().await.expect("sink2: no events");

    assert_eq!(r1.len(), 1);
    assert_eq!(r2.len(), 1);

    loader.shutdown().await.expect("shutdown failed");
}

#[tokio::test]
async fn test_builder_rejects_no_source() {
    let (sink, _rx) = ChannelSink::new(1);
    let result = DataLoader::<MockSource>::builder().sink(sink).build();

    assert!(result.is_err());
    let err_msg = format!("{}", result.err().unwrap());
    assert!(err_msg.contains("no data source"));
}

#[tokio::test]
async fn test_builder_rejects_no_sinks() {
    let source = MockSource::new(vec![]);
    let result = DataLoader::builder().source(source).build();

    assert!(result.is_err());
    let err_msg = format!("{}", result.err().unwrap());
    assert!(err_msg.contains("at least one sink"));
}

#[tokio::test]
async fn test_loader_cancel_token() {
    let (sink, _rx) = ChannelSink::new(1);
    let source = MockSource::new(vec![]);

    let loader = DataLoader::builder()
        .source(source)
        .sink(sink)
        .build()
        .expect("build failed");

    let token = loader.cancel_token();
    assert!(!token.is_cancelled());
}

#[tokio::test]
async fn test_start_twice_errors() {
    let (sink, mut rx) = ChannelSink::new(1);

    let events = vec![make_candle(
        5000, "GOOGL", 140.0, 142.0, 139.0, 141.0, 800_000.0,
    )];
    let source_clone = MockSource::new(vec![events]);

    let mut loader = DataLoader::builder()
        .source(source_clone)
        .sink(sink)
        .build()
        .expect("build failed");

    loader.start().await.expect("first start");

    // Drain the events to avoid channel issues
    let _ = rx.recv().await;
    loader.shutdown().await.expect("first shutdown");

    // Starting a second time should fail because source is consumed.
    let result = loader.start().await;
    assert!(result.is_err());
}

// ------------------------------------------------------------------
// Event model tests
// ------------------------------------------------------------------

#[test]
fn test_candle_data_helpers() {
    let c = CandleData::new(
        1_700_000_000,
        "AAPL",
        "1D",
        150.0,
        155.0,
        149.0,
        153.0,
        1_000_000.0,
    );

    assert!(c.is_bullish());
    assert!(!c.is_bearish());
    assert!((c.typical_price() - (155.0 + 149.0 + 153.0) / 3.0).abs() < 1e-10);

    let dt = c.datetime().expect("valid timestamp");
    assert_eq!(dt.timestamp(), 1_700_000_000);
}

#[test]
fn test_candle_data_bearish() {
    let c = CandleData::new(
        1_700_000_000,
        "AAPL",
        "1D",
        155.0,
        156.0,
        149.0,
        150.0,
        500_000.0,
    );

    assert!(!c.is_bullish());
    assert!(c.is_bearish());
}

#[test]
fn test_market_event_timestamp() {
    let candle = make_candle(999, "X", 1.0, 2.0, 1.0, 2.0, 1.0);
    assert_eq!(candle.timestamp(), 999);
}

#[test]
fn test_market_event_symbol() {
    let candle = make_candle(1, "TSLA", 1.0, 2.0, 1.0, 2.0, 1.0);
    assert_eq!(candle.symbol(), Some("TSLA"));
}
