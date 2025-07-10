use serde::Deserialize;
use serde_json::Value;
use tracing::{debug, error, info, warn};
use ustr::Ustr;

use crate::{
    ChartResponseData, Error, QuoteData, QuoteValue, Result, StudyOptions, StudyResponseData,
    SymbolInfo,
    error::TradingViewError,
    live::{handler::types::TradingViewHandler, models::TradingViewDataEvent},
    quote::utils::merge_quotes,
    websocket::Metadata,
};

#[derive(Clone, Default)]
pub struct Handler {
    pub(crate) metadata: Metadata,
    pub(crate) handler: TradingViewHandler,
}

impl Handler {
    #[tracing::instrument(skip(self, message), level = "debug")]
    pub(crate) async fn handle_events(&self, event: TradingViewDataEvent, message: &[Value]) {
        if let Err(e) = self.process_event(event, message).await {
            error!("Event processing error: {:?}", e);
            self.notify_error(e, message);
        }
    }

    async fn process_event(&self, event: TradingViewDataEvent, message: &[Value]) -> Result<()> {
        match event {
            TradingViewDataEvent::OnChartData | TradingViewDataEvent::OnChartDataUpdate => {
                self.handle_chart_data(message).await?;
                Ok(())
            }
            TradingViewDataEvent::OnQuoteData => {
                self.handle_quote_data(message).await;
                Ok(())
            }
            TradingViewDataEvent::OnSymbolResolved => self.handle_symbol_resolved(message).await,
            TradingViewDataEvent::OnSeriesCompleted => {
                (self.handler.on_series_completed)(message.to_vec());
                Ok(())
            }
            TradingViewDataEvent::OnSeriesLoading => {
                (self.handler.on_series_loading)(message.to_vec());
                Ok(())
            }
            TradingViewDataEvent::OnQuoteCompleted => {
                (self.handler.on_quote_completed)(message.to_vec());
                Ok(())
            }
            TradingViewDataEvent::OnReplayOk => {
                (self.handler.on_replay_ok)(message.to_vec());
                Ok(())
            }
            TradingViewDataEvent::OnReplayPoint => {
                (self.handler.on_replay_point)(message.to_vec());
                Ok(())
            }
            TradingViewDataEvent::OnReplayInstanceId => {
                (self.handler.on_replay_instance_id)(message.to_vec());
                Ok(())
            }
            TradingViewDataEvent::OnReplayResolutions => {
                (self.handler.on_replay_resolutions)(message.to_vec());
                Ok(())
            }
            TradingViewDataEvent::OnReplayDataEnd => {
                (self.handler.on_replay_data_end)(message.to_vec());
                Ok(())
            }
            TradingViewDataEvent::OnStudyLoading => {
                (self.handler.on_study_loading)(message.to_vec());
                Ok(())
            }
            TradingViewDataEvent::OnStudyCompleted => {
                (self.handler.on_study_completed)(message.to_vec());
                Ok(())
            }
            TradingViewDataEvent::OnError(error) => {
                let error = Error::TradingView { source: error };
                self.notify_error(error, message);
                Ok(())
            }
            TradingViewDataEvent::UnknownEvent(event) => {
                (self.handler.on_unknown_event)((event, message.to_vec()));
                Ok(())
            }
        }
    }

    async fn handle_symbol_resolved(&self, message: &[Value]) -> Result<()> {
        if message.len() <= 2 {
            return Ok(());
        }

        let symbol_info = SymbolInfo::deserialize(&message[2])?;
        debug!("receive symbol info: {:?}", symbol_info);

        {
            let mut chart_state = self.metadata.chart_state.write().await;
            chart_state.symbol_info.replace(symbol_info.clone());
        }

        (self.handler.on_symbol_info)(symbol_info);
        Ok(())
    }

    async fn handle_study_data(&self, options: &StudyOptions, message_data: &Value) -> Result<()> {
        // Pre-allocate vector if we know the capacity
        let studies_len = self.metadata.studies.len();
        if studies_len == 0 {
            return Ok(());
        }

        for study_entry in self.metadata.studies.iter() {
            let (k, v) = study_entry.pair();
            if let Some(resp_data) = message_data.get(v.as_str()) {
                // Avoid debug logging in hot path unless explicitly enabled
                if tracing::enabled!(tracing::Level::DEBUG) {
                    debug!("study data received: {} - {:?}", k, resp_data);
                }
                let data = StudyResponseData::deserialize(resp_data)?;
                (self.handler.on_study_data)((*options, data));
            }
        }
        Ok(())
    }

    async fn handle_chart_data(&self, message: &[Value]) -> Result<()> {
        if message.len() < 2 {
            return Ok(());
        }

        let message_data = &message[1];

        // Early return if no series to process
        if self.metadata.series.is_empty() {
            return Ok(());
        }

        // Process each series
        for series_entry in self.metadata.series.iter() {
            let (id, series_info) = series_entry.pair();

            if let Some(resp_data) = message_data.get(id.as_str()) {
                let chart_response = ChartResponseData::deserialize(resp_data)?;
                let data = chart_response.series;

                // Clone series_info once outside the lock
                let series_info_clone = series_info.clone();
                let data_clone = data.clone();

                // Update chart state with minimal lock scope
                let chart_data = {
                    let mut chart_state = self.metadata.chart_state.write().await;
                    chart_state
                        .chart
                        .replace((series_info_clone.clone(), data_clone.clone()));
                    (series_info_clone, data_clone)
                };

                (self.handler.on_chart_data)(chart_data);

                // Handle study data if present
                if let Some(study_options) = &series_info.options.study_config {
                    self.handle_study_data(study_options, message_data).await?;
                }
            }
        }

        Ok(())
    }

    async fn handle_quote_data(&self, message: &[Value]) {
        if message.len() < 2 {
            warn!("Quote message too short: {}", message.len());
            return;
        }

        // Reduce debug logging overhead
        if tracing::enabled!(tracing::Level::DEBUG) {
            debug!("Processing quote data message: {:?}", message);
        }

        // Try primary format first
        if self.try_parse_quote_data(&message[1]).await.is_ok() {
            return;
        }

        // Try alternative format
        if let Err(e) = self.try_parse_direct_quote(&message[1]).await {
            error!("All quote parsing attempts failed: {:?}", e);
            self.notify_error(
                Error::JsonParse(Ustr::from("Failed to parse quote data")),
                message,
            );
        }
    }

    async fn try_parse_quote_data(&self, data: &Value) -> Result<()> {
        let qsd = QuoteData::deserialize(data)
            .map_err(|e| Error::JsonParse(Ustr::from(&e.to_string())))?;

        // Reduce info logging overhead in hot paths
        if tracing::enabled!(tracing::Level::INFO) {
            info!("Successfully parsed quote data: {:?}", qsd);
        }

        if qsd.status != "ok" {
            return Err(Error::TradingView {
                source: TradingViewError::QuoteDataStatusError(Ustr::from(&qsd.status)),
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

        (self.handler.on_quote_data)(value);
        Ok(())
    }

    async fn try_parse_direct_quote(&self, data: &Value) -> Result<()> {
        let direct_quote = QuoteValue::deserialize(data)
            .map_err(|e| Error::JsonParse(Ustr::from(&e.to_string())))?;

        if tracing::enabled!(tracing::Level::INFO) {
            info!("Parsed as direct QuoteValue: {:?}", direct_quote);
        }
        (self.handler.on_quote_data)(direct_quote);
        Ok(())
    }

    fn notify_error(&self, error: Error, message: &[Value]) {
        (self.handler.on_error)((error, message.to_vec()));
    }

    pub fn set_handler(mut self, handler: TradingViewHandler) -> Self {
        self.handler = handler;
        self
    }
}
