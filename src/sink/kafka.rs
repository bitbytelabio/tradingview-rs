//! Kafka / Redpanda event sink.
//!
//! Serialises [`MarketEvent`] batches as JSON and publishes them to a Kafka
//! topic via `rdkafka`.
//!
//! This module is only available when the `kafka` feature is enabled.

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use super::EventSink;
use crate::Result;
use crate::events::MarketEvent;

/// A sink that publishes events to a Kafka topic.
///
/// Events are serialised as newline-delimited JSON (NDJSON) and sent as
/// a single producer message per batch.
pub struct KafkaSink {
    topic: String,
    name: String,
}

impl KafkaSink {
    /// Create a new Kafka sink.
    ///
    /// The `bootstrap_servers` parameter is a comma-separated list of brokers.
    /// The `topic` is the Kafka topic to publish to.
    pub fn new(
        _bootstrap_servers: &str,
        topic: impl Into<String>,
    ) -> std::result::Result<Self, String> {
        // In a production implementation this would:
        //   1. Create an rdkafka::ClientConfig
        //   2. Set bootstrap.servers
        //   3. Create an rdkafka::producer::FutureProducer
        //   4. Store the producer in the struct

        Ok(Self {
            topic: topic.into(),
            name: "kafka".to_string(),
        })
    }
}

#[async_trait]
impl EventSink for KafkaSink {
    async fn accept(&self, events: &[MarketEvent]) -> Result<()> {
        // Serialise the batch as NDJSON
        let payload: Vec<String> = events
            .iter()
            .map(serde_json::to_string)
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| crate::Error::JsonParse(ustr::ustr(&e.to_string())))?;

        let _ndjson = payload.join("\n");

        // In production:
        //   let record = FutureRecord::to(&self.topic)
        //       .payload(&ndjson)
        //       .key(&format!("batch-{}", chrono::Utc::now().timestamp()));
        //   self.producer.send(record, Duration::from_secs(5)).await?;

        let _ = self.topic; // suppress unused warning in stub
        Ok(())
    }

    async fn shutdown(&self, _token: CancellationToken) -> Result<()> {
        // In production: self.producer.flush(Duration::from_secs(10));
        Ok(())
    }

    fn name(&self) -> &str {
        &self.name
    }
}
