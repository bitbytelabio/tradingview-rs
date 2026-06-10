//! Example: callback sink consumer.
//!
//! Demonstrates using a `CallbackSink` to process events with a closure.

use async_trait::async_trait;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tradingview::Result;
use tradingview::events::{CandleData, MarketEvent};
use tradingview::loader::DataLoader;
use tradingview::sink::callback::CallbackSink;
use tradingview::source::DataSource;

// ---------------------------------------------------------------------------
// Mock source
// ---------------------------------------------------------------------------

struct ExampleSource;

#[async_trait]
impl DataSource for ExampleSource {
    async fn run(
        &self,
        sink: mpsc::Sender<Vec<MarketEvent>>,
        cancel: CancellationToken,
    ) -> Result<()> {
        for i in 0..5 {
            if cancel.is_cancelled() {
                break;
            }
            let ts = 1_700_000_000 + i * 3600;
            let batch = vec![MarketEvent::Candle(CandleData::new(
                ts,
                "NASDAQ:AAPL",
                "1h",
                195.0 + i as f64,
                197.0 + i as f64,
                194.0 + i as f64,
                196.0 + i as f64,
                5_000_000.0,
            ))];
            let _ = sink.send(batch).await;
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        }
        Ok(())
    }

    fn name(&self) -> &str {
        "example-callback-source"
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    // Track how many events we've seen
    let count = Arc::new(AtomicUsize::new(0));
    let count_clone = Arc::clone(&count);

    // Create a callback sink
    let callback = CallbackSink::new("printer", move |events: Vec<MarketEvent>| {
        let c = Arc::clone(&count_clone);
        async move {
            for event in &events {
                if let MarketEvent::Candle(candle) = event {
                    let n = c.fetch_add(1, Ordering::SeqCst);
                    println!(
                        "#{} [{}] {} C={:.2} V={:.0}",
                        n, candle.timestamp, candle.symbol, candle.close, candle.volume,
                    );
                }
            }
            Ok(())
        }
    });

    let mut loader = DataLoader::builder()
        .source(ExampleSource)
        .sink(callback)
        .build()?;

    loader.start().await?;
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    loader.shutdown().await?;

    println!("Processed {} events total.", count.load(Ordering::SeqCst));

    Ok(())
}
