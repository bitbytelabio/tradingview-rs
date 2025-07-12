use dashmap::DashMap;
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;
use ustr::{Ustr, ustr};

use crate::{
    Error, QuoteData, QuoteValue, Result, SeriesDataResponse,
    error::TradingViewError,
    live::{handler::types::EventHandler, models::TradingViewDataEvent},
    quote::utils::merge_quotes,
};

#[derive(Default, Clone)]
pub(crate) struct Metadata {
    pub(crate) quotes: Arc<DashMap<Ustr, QuoteValue>>,
}

#[derive(Clone)]
pub struct Handler {
    pub(crate) metadata: Metadata,
    pub(crate) event_handler: EventHandler,
}

impl Handler {
    #[tracing::instrument(skip(self, message), level = "debug")]
    pub(crate) async fn handle_events(&self, event: TradingViewDataEvent, message: &[Value]) {
        if let Err(e) = self.process_event(event, message).await {
            tracing::error!("Event processing error: {:?}", e);
            let error_event = format!("Error processing event: {:?}", e);
            let error_message = format!("{:?}", message);
            self.notify_error(
                e,
                json!({
                    "event": error_event,
                    "message": error_message,
                })
                .as_array()
                .unwrap_or(&vec![]),
            );
        }
    }

    async fn process_event(&self, event: TradingViewDataEvent, messages: &[Value]) -> Result<()> {
        match event {
            TradingViewDataEvent::OnError(error) => {
                (self.event_handler.on_tradingview_error)((error, messages.to_vec()));
                Ok(())
            }
            TradingViewDataEvent::OnQuoteData => {
                self.handle_quote_data(messages).await;
                Ok(())
            }
            TradingViewDataEvent::OnChartData | TradingViewDataEvent::OnChartDataUpdate => {
                if messages.is_empty() {
                    tracing::warn!("Received empty message for event: {:?}", event);
                    return Ok(());
                }
                self.handle_series_data(event, messages).await?;
                Ok(())
            }
            _ => {
                (self.event_handler.on_signal)((event, messages.to_vec()));
                Ok(())
            }
        }
    }

    async fn handle_series_data(
        &self,
        event: TradingViewDataEvent,
        messages: &[Value],
    ) -> Result<()> {
        if messages.is_empty() {
            tracing::warn!("Received empty message for event: {:?}", event);
            return Ok(());
        }

        let mut series_data = Vec::new();

        for message in messages {
            if message.is_null() {
                tracing::warn!("Received null message for event: {:?}", event);
                continue;
            }

            let value = SeriesDataResponse::deserialize(message)?;
            series_data.push(value);
        }

        (self.event_handler.on_series_data)((event, series_data));

        Ok(())
    }

    async fn handle_quote_data(&self, message: &[Value]) {
        if message.len() < 2 {
            tracing::warn!("Quote message too short: {}", message.len());
            return;
        }

        // Reduce debug logging overhead
        if tracing::enabled!(tracing::Level::DEBUG) {
            tracing::debug!("Processing quote data message: {:?}", message);
        }

        // Try primary format first
        if self.try_parse_quote_data(&message[1]).await.is_ok() {
            return;
        }

        // Try alternative format
        if let Err(e) = self.try_parse_direct_quote(&message[1]).await {
            tracing::error!("All quote parsing attempts failed: {:?}", e);
            self.notify_error(
                Error::JsonParse(ustr("Failed to parse quote data")),
                message,
            );
        }
    }

    async fn try_parse_quote_data(&self, data: &Value) -> Result<()> {
        let qsd =
            QuoteData::deserialize(data).map_err(|e| Error::JsonParse(ustr(&e.to_string())))?;

        // Reduce info logging overhead in hot paths
        if tracing::enabled!(tracing::Level::INFO) {
            tracing::info!("Successfully parsed quote data: {:?}", qsd);
        }

        if qsd.status != "ok" {
            return Err(Error::TradingView {
                source: TradingViewError::QuoteDataStatusError(ustr(&qsd.status)),
            });
        }

        // Optimize quote storage update with entry API
        let name = qsd.name;
        let value = qsd.value;

        match self.metadata.quotes.get_mut(&name) {
            Some(mut prev_quote) => {
                *prev_quote = merge_quotes(&prev_quote, &value);
            }
            None => {
                self.metadata.quotes.insert(name, value);
            }
        }

        (self.event_handler.on_quote_data)(value);
        Ok(())
    }

    async fn try_parse_direct_quote(&self, data: &Value) -> Result<()> {
        let direct_quote =
            QuoteValue::deserialize(data).map_err(|e| Error::JsonParse(ustr(&e.to_string())))?;

        if tracing::enabled!(tracing::Level::INFO) {
            tracing::info!("Parsed as direct QuoteValue: {:?}", direct_quote);
        }
        (self.event_handler.on_quote_data)(direct_quote);
        Ok(())
    }

    fn notify_error(&self, error: Error, message: &[Value]) {
        (self.event_handler.on_internal_error)((error, message.to_vec()));
    }

    pub fn set_handler(mut self, handler: EventHandler) -> Self {
        self.event_handler = handler;
        self
    }
}
