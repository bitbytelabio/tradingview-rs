use serde::Deserialize;
use serde_json::Value;
use std::sync::Arc;
use ustr::Ustr;

use crate::{
    ChartResponseData, Error, QuoteData, QuoteValue, Result, StudyOptions, StudyResponseData,
    SymbolInfo,
    error::TradingViewError,
    live::{
        handler::types::{DataTx, TradingViewHandler, create_handler},
        socket::TradingViewDataEvent,
    },
    quote::utils::merge_quotes,
    websocket::Metadata,
};

#[derive(Clone, Default)]
pub struct DataHandler {
    pub(crate) metadata: Metadata,
    pub(crate) handler: TradingViewHandler,
}

#[bon::bon]
impl DataHandler {
    #[builder]
    pub fn new(res_tx: DataTx) -> Self {
        let res_tx = Arc::new(res_tx);
        let handler: TradingViewHandler = create_handler(res_tx);
        Self {
            metadata: Metadata::default(),
            handler,
        }
    }

    pub(crate) async fn handle_events(&self, event: TradingViewDataEvent, message: &Vec<Value>) {
        match event {
            TradingViewDataEvent::OnChartData | TradingViewDataEvent::OnChartDataUpdate => {
                tracing::trace!("received raw chart data: {:?}", message);
                // Avoid cloning the entire HashMap
                if let Err(e) = self.handle_chart_data(message).await {
                    tracing::error!("chart data parsing error: {:?}", e);
                    (self.handler.on_error)((e, message.to_owned()));
                }
            }
            TradingViewDataEvent::OnQuoteData => self.handle_quote_data(message).await,
            TradingViewDataEvent::OnSymbolResolved => {
                if message.len() > 2 {
                    match SymbolInfo::deserialize(&message[2]) {
                        Ok(s) => {
                            tracing::debug!("receive symbol info: {:?}", s);
                            let mut chart_state = self.metadata.chart_state.write().await;
                            chart_state.symbol_info.replace(s.clone());
                            (self.handler.on_symbol_info)(s);
                        }
                        Err(e) => {
                            tracing::error!("symbol resolved parsing error: {:?}", e);
                            (self.handler.on_error)((
                                Error::JsonParse(Ustr::from(&e.to_string())),
                                message.to_owned(),
                            ));
                        }
                    }
                }
            }
            TradingViewDataEvent::OnSeriesCompleted => {
                tracing::debug!("series completed: {:?}", message);
                (self.handler.on_series_completed)(message.to_owned());
            }
            TradingViewDataEvent::OnSeriesLoading => {
                tracing::debug!("series loading: {:?}", message);
                (self.handler.on_series_loading)(message.to_owned());
            }
            TradingViewDataEvent::OnQuoteCompleted => {
                tracing::debug!("quote completed: {:?}", message);
                (self.handler.on_quote_completed)(message.to_owned());
            }
            TradingViewDataEvent::OnReplayOk => {
                tracing::debug!("replay ok: {:?}", message);
                (self.handler.on_replay_ok)(message.to_owned());
            }
            TradingViewDataEvent::OnReplayPoint => {
                tracing::debug!("replay point: {:?}", message);
                (self.handler.on_replay_point)(message.to_owned());
            }
            TradingViewDataEvent::OnReplayInstanceId => {
                tracing::debug!("replay instance id: {:?}", message);
                (self.handler.on_replay_instance_id)(message.to_owned());
            }
            TradingViewDataEvent::OnReplayResolutions => {
                tracing::debug!("replay resolutions: {:?}", message);
                (self.handler.on_replay_resolutions)(message.to_owned());
            }
            TradingViewDataEvent::OnReplayDataEnd => {
                tracing::debug!("replay data end: {:?}", message);
                (self.handler.on_replay_data_end)(message.to_owned());
            }
            TradingViewDataEvent::OnStudyLoading => {
                tracing::debug!("study loading: {:?}", message);
                (self.handler.on_study_loading)(message.to_owned());
            }
            TradingViewDataEvent::OnStudyCompleted => {
                tracing::debug!("study completed: {:?}", message);
                (self.handler.on_study_completed)(message.to_owned());
            }
            TradingViewDataEvent::OnError(tradingview_error) => {
                tracing::error!("trading view error: {:?}", tradingview_error);
                (self.handler.on_error)((
                    Error::TradingView {
                        source: tradingview_error,
                    },
                    message.to_owned(),
                ));
            }
            TradingViewDataEvent::UnknownEvent(event) => {
                tracing::warn!("unknown event: {:?}", event);
                (self.handler.on_unknown_event)((event, message.to_owned()));
            }
        }
    }

    async fn handle_study_data(&self, options: &StudyOptions, message_data: &Value) -> Result<()> {
        for study_entry in self.metadata.studies.iter() {
            let (k, v) = study_entry.pair();
            if let Some(resp_data) = message_data.get(v.as_str()) {
                tracing::debug!("study data received: {} - {:?}", k, resp_data);
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

        // Process each series without cloning the entire series map
        for series_entry in self.metadata.series.iter() {
            let (id, series_info) = series_entry.pair();

            if let Some(resp_data) = message_data.get(id.as_str()) {
                let data = ChartResponseData::deserialize(resp_data)?.series;

                // Update chart state
                {
                    let mut chart_state = self.metadata.chart_state.write().await;
                    chart_state
                        .chart
                        .replace((series_info.clone(), data.clone()));
                }

                // Read once for callback
                let chart_data = self
                    .metadata
                    .chart_state
                    .read()
                    .await
                    .chart
                    .clone()
                    .unwrap_or_default();
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
            return;
        }

        tracing::debug!("received raw quote data: {:?}", message);
        let qsd = match QuoteData::deserialize(&message[1]) {
            Ok(data) => data,
            Err(_) => return,
        };

        if qsd.status == "ok" {
            self.metadata
                .quotes
                .entry(qsd.name)
                .and_modify(|prev_quote| {
                    *prev_quote = merge_quotes(prev_quote, &qsd.value);
                })
                .or_insert(qsd.value);

            // Collect quotes once to avoid multiple read locks
            let quotes: Vec<QuoteValue> = self
                .metadata
                .quotes
                .iter()
                .map(|entry| *entry.value())
                .collect();
            for quote in quotes {
                (self.handler.on_quote_data)(quote);
            }
        } else {
            tracing::error!("quote data status error: {:?}", qsd);
            (self.handler.on_error)((
                Error::TradingView {
                    source: TradingViewError::QuoteDataStatusError(Ustr::from(&qsd.status)),
                },
                message.to_owned(),
            ));
        }
    }

    pub fn set_handler(mut self, handler: TradingViewHandler) -> Self {
        self.handler = handler;
        self
    }
}
