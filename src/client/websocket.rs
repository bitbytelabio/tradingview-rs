use crate::{
    callback::Callbacks,
    chart::{
        models::{ChartResponseData, StudyResponseData, SymbolInfo},
        session::SeriesInfo,
        StudyOptions,
    },
    error::TradingViewError,
    quote::{
        models::{QuoteData, QuoteValue},
        utils::merge_quotes,
    },
    socket::TradingViewDataEvent,
    Error, Result,
};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use tracing::{debug, error, trace};

#[derive(Clone, Default)]
pub struct WSClient<'a> {
    pub(crate) metadata: Metadata,
    pub(crate) callbacks: Callbacks<'a>,
}

#[derive(Default, Clone)]
pub struct Metadata {
    pub series_count: u16,
    pub series: HashMap<String, SeriesInfo>,
    pub studies_count: u16,
    pub studies: HashMap<String, String>,
    pub quotes: HashMap<String, QuoteValue>,
    pub quote_session: String,
}

impl<'a> WSClient<'a> {
    pub(crate) async fn handle_events(
        &mut self,
        event: TradingViewDataEvent,
        message: &Vec<Value>,
    ) {
        match event {
            TradingViewDataEvent::OnChartData | TradingViewDataEvent::OnChartDataUpdate => {
                trace!("received raw chart data: {:?}", message);
                match self
                    .handle_chart_data(&self.metadata.series, &self.metadata.studies, message)
                    .await
                {
                    Ok(_) => (),
                    Err(e) => {
                        error!("chart data parsing error: {:?}", e);
                        (self.callbacks.on_error)(e).await;
                    }
                };
            }
            TradingViewDataEvent::OnQuoteData => self.handle_quote_data(message).await,
            TradingViewDataEvent::OnSymbolResolved => {
                match SymbolInfo::deserialize(&message[2]) {
                    Ok(s) => {
                        debug!("receive symbol info: {:?}", s);
                        (self.callbacks.on_symbol_info)(s).await;
                    }
                    Err(e) => {
                        error!("symbol resolved parsing error: {:?}", e);
                        (self.callbacks.on_error)(Error::JsonParseError(e)).await;
                    }
                };
            }
            _ => {
                debug!("event: {:?}, message: {:?}", event, message);
                (self.callbacks.on_other_event)((event, message.to_owned())).await;
            }
        }
    }

    async fn handle_chart_data(
        &self,
        series: &HashMap<String, SeriesInfo>,
        studies: &HashMap<String, String>,
        message: &[Value],
    ) -> Result<()> {
        for (id, s) in series.iter() {
            debug!("received raw message - v: {:?}, m: {:?}", s, message);
            match message[1].get(id.as_str()) {
                Some(resp_data) => {
                    let data = ChartResponseData::deserialize(resp_data)?.series;
                    debug!("series data extracted: {:?}", data);
                    (self.callbacks.on_chart_data)((s.options.clone(), data)).await;
                }
                None => {
                    debug!("receive empty data on series: {:?}", s);
                }
            }

            if let Some(study_options) = &s.options.study_config {
                self.handle_study_data(study_options, studies, message)
                    .await?;
            }
        }
        Ok(())
    }

    async fn handle_study_data(
        &self,
        options: &StudyOptions,
        studies: &HashMap<String, String>,
        message: &[Value],
    ) -> Result<()> {
        for (k, v) in studies.iter() {
            if let Some(resp_data) = message[1].get(v.as_str()) {
                debug!("study data received: {} - {:?}", k, resp_data);
                let data = StudyResponseData::deserialize(resp_data)?;
                (self.callbacks.on_study_data)((options.clone(), data)).await;
            }
        }
        Ok(())
    }

    async fn handle_quote_data(&mut self, message: &[Value]) {
        debug!("received raw quote data: {:?}", message);
        let qsd = QuoteData::deserialize(&message[1]).unwrap_or_default();
        if qsd.status == "ok" {
            if let Some(prev_quote) = self.metadata.quotes.get_mut(&qsd.name) {
                *prev_quote = merge_quotes(prev_quote, &qsd.value);
            } else {
                self.metadata.quotes.insert(qsd.name, qsd.value);
            }

            for q in self.metadata.quotes.values() {
                debug!("quote data: {:?}", q);
                (self.callbacks.on_quote_data)(q.to_owned()).await;
            }
        } else {
            error!("quote data status error: {:?}", qsd);
            (self.callbacks.on_error)(Error::TradingViewError(
                TradingViewError::QuoteDataStatusError,
            ))
            .await;
        }
    }

    pub fn set_callbacks(mut self, callbacks: Callbacks<'a>) -> Self {
        self.callbacks = callbacks;
        self
    }
}
