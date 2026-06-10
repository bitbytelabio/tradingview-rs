//! Example: channel sink consumer.
//!
//! Demonstrates using a `ChannelSink` to receive market events from the loader
//! in a separate task.

use async_trait::async_trait;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tradingview::Result;
use tradingview::events::MarketEvent;
use tradingview::loader::DataLoader;
use tradingview::sink::channel::ChannelSink;
use tradingview::source::DataSource;

// ---------------------------------------------------------------------------
// Mock source for examples
// ---------------------------------------------------------------------------

struct ExampleSource;

#[async_trait]
impl DataSource for ExampleSource {
    async fn run(
        &self,
        sink: mpsc::Sender<Vec<MarketEvent>>,
        cancel: CancellationToken,
    ) -> Result<()> {
        // Simulate a data feed producing candles
        use tradingview::events::CandleData;

        for i in 0..10 {
            if cancel.is_cancelled() {
                break;
            }

            let ts = 1_700_000_000 + i * 60;
            let batch = vec![MarketEvent::Candle(CandleData::new(
                ts,
                "BINANCE:BTCUSDT",
                "1m",
                50000.0 + i as f64 * 10.0,
                50100.0 + i as f64 * 10.0,
                49900.0 + i as f64 * 10.0,
                50050.0 + i as f64 * 10.0,
                100.0 + i as f64,
            ))];

            if sink.send(batch).await.is_err() {
                break;
            }

            // Simulate real-time feed delay
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }

        Ok(())
    }

    fn name(&self) -> &str {
        "example-source"
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    // Create channel sink to receive events
    let (channel_sink, mut rx) = ChannelSink::new(1024);

    // Build and start the loader
    let mut loader = DataLoader::builder()
        .source(ExampleSource)
        .sink(channel_sink)
        .build()?;

    loader.start().await?;

    // Consumer task: receive events from the channel
    let consumer = tokio::spawn(async move {
        while let Some(batch) = rx.recv().await {
            for event in &batch {
                if let MarketEvent::Candle(c) = event {
                    println!(
                        "[{}] {} O={:.2} H={:.2} L={:.2} C={:.2} V={:.2}",
                        c.timestamp, c.symbol, c.open, c.high, c.low, c.close, c.volume,
                    );
                }
            }
        }
    });

    // Let the data flow for a while, then shutdown
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    println!("Shutting down...");
    loader.shutdown().await?;
    consumer.await?;

    println!("Example complete.");
    Ok(())
}
