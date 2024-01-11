use crate::{
    callback::Callbacks,
    chart::{
        models::{ChartResponseData, StudyResponseData, SymbolInfo},
        session::SeriesInfo,
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
use tracing::{debug, error, info, trace, warn};

#[derive(Clone, Default)]
pub struct DataLoader<'a> {
    pub(crate) metadata: Metadata,
    pub(crate) callbacks: Callbacks<'a>,
}

#[derive(Default, Debug, Clone)]
pub struct DataLoaderBuilder {}

#[derive(Default, Clone)]
pub struct Metadata {
    pub series_count: u16,
    pub series: HashMap<String, SeriesInfo>,
    pub studies_count: u16,
    pub studies: HashMap<String, String>,
    pub quotes: HashMap<String, QuoteValue>,
    pub quote_session: String,
}

impl<'a> DataLoader<'a> {
    pub(crate) async fn handle_events(
        &mut self,
        event: TradingViewDataEvent,
        message: &Vec<Value>,
    ) {
        match event {
            TradingViewDataEvent::OnChartData | TradingViewDataEvent::OnChartDataUpdate => {
                trace!("received chart data: {:?}", message);
                match self
                    .handle_chart_data(&self.metadata.series, &self.metadata.studies, message)
                    .await
                {
                    Ok(_) => (),
                    Err(e) => {
                        (self.callbacks.on_error)(e).await;
                    }
                };
            }
            TradingViewDataEvent::OnQuoteData => self.handle_quote_data(message).await,
            TradingViewDataEvent::OnQuoteCompleted => {
                info!("quote completed: {:?}", message)
            }
            TradingViewDataEvent::OnSeriesLoading => {
                trace!("series is loading: {:#?}", message);
            }
            TradingViewDataEvent::OnSeriesCompleted => {
                // let message = SeriesCompletedMessage {
                //     session: get_string_value(&message, 0),
                //     id: get_string_value(&message, 1),
                //     update_mode: get_string_value(&message, 2),
                //     version: get_string_value(&message, 3),
                // };

                info!("series completed: {:#?}", message);
            }
            TradingViewDataEvent::OnSymbolResolved => {
                match SymbolInfo::deserialize(&message[2]) {
                    Ok(s) => debug!("{:?}", s),
                    Err(e) => {
                        error!("{:?}", e);
                        // return SymbolInfo::default();
                    }
                };
                // info!("symbol resolved: {:?}", &message[2]);
                // debug!("{:?}", symbol_info)
                // let symbol_info = serde_json::from_value::<SymbolInfo>(message[2].clone())?;
            }
            TradingViewDataEvent::OnReplayOk => {
                info!("replay ok: {:?}", message);
            }
            TradingViewDataEvent::OnReplayPoint => {
                info!("replay point: {:?}", message);
            }
            TradingViewDataEvent::OnReplayInstanceId => {
                info!("replay instance id: {:?}", message);
            }
            TradingViewDataEvent::OnReplayResolutions => todo!("8"),
            TradingViewDataEvent::OnReplayDataEnd => todo!("9"),
            TradingViewDataEvent::OnStudyLoading => todo!("10"),
            TradingViewDataEvent::OnStudyCompleted => {
                info!("study completed: {:?}", message);
            }
            TradingViewDataEvent::OnError(e) => error!("error: {:?}", e),
            TradingViewDataEvent::UnknownEvent(_) => todo!("13"),
        }
    }

    async fn handle_chart_data(
        &self,
        series: &HashMap<String, SeriesInfo>,
        studies: &HashMap<String, String>,
        message: &[Value],
    ) -> Result<()> {
        for (id, s) in series.iter() {
            trace!("received v: {:?}, m: {:?}", s, message);
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
        }
        self.handle_study_data(studies, message).await?;
        Ok(())
    }

    async fn handle_study_data(
        &self,
        studies: &HashMap<String, String>,
        message: &[Value],
    ) -> Result<()> {
        for (k, v) in studies.iter() {
            if let Some(resp_data) = message[1].get(v.as_str()) {
                debug!("study data received: {} - {:?}", k, resp_data);
                let data = StudyResponseData::deserialize(resp_data)?;
                warn!("study data extracted: {} - {:?}", k, data);

                // TODO: Notify function
            }
        }
        Ok(())
    }

    async fn handle_quote_data(&mut self, message: &[Value]) {
        let qsd = QuoteData::deserialize(&message[1]).unwrap();
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
}
