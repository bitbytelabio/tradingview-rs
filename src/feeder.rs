use std::{ collections::HashMap, sync::Arc };
use serde::Deserialize;
use serde_json::Value;
use tracing::{ debug, error, info, trace, warn };

use crate::{
    Result,
    Error,
    quote::{ models::{ QuoteData, QuoteValue }, utils::merge_quotes },
    socket::{ Socket, TradingViewDataEvent, SocketMessageDe, SocketSession },
    chart::{
        session::SeriesInfo,
        models::{ SymbolInfo, ChartResponseData, SeriesCompletedMessage, StudyResponseData },
    },
};

#[derive(Clone)]
pub struct Publisher {
    pub metadata: Metadata,
    // publisher: Vec<Box<dyn Socket + Send + Sync>>,
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

impl Publisher {
    // pub async fn subscribe<T: Socket + Send + Sync>(
    //     &mut self,
    //     listener: &mut T,
    //     socket: &mut SocketSession
    // ) {
    //     listener.event_loop(&mut socket.clone()).await;
    // }

    pub fn new() -> Self {
        Self {
            metadata: Metadata::default(),
            // publisher: Vec::new(),
        }
    }

    // pub fn add(mut self, publisher: Box<dyn Socket + Send + Sync>) -> Self {
    //     self.publisher.push(publisher);
    //     self
    // }

    pub fn unsubscribe<T: Socket + Send>(
        &mut self,
        event_type: TradingViewDataEvent,
        listener: T
    ) {}

    pub fn notify(&self, event_type: TradingViewDataEvent) {}

    pub async fn handle_events(&mut self, event: TradingViewDataEvent, message: &Vec<Value>) {
        match event {
            TradingViewDataEvent::OnChartData | TradingViewDataEvent::OnChartDataUpdate => {
                trace!("received chart data: {:?}", message);

                match
                    self.handle_chart_data(
                        &self.metadata.series,
                        &self.metadata.studies,
                        message
                    ).await
                {
                    Ok(_) => (),
                    Err(e) => error!("{}", e),
                };
            }
            TradingViewDataEvent::OnQuoteData => { self.handle_quote_data(message).await }
            TradingViewDataEvent::OnQuoteCompleted => { info!("quote completed: {:?}", message) }
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
                let symbol_info = match SymbolInfo::deserialize(&message[2]) {
                    Ok(s) => s,
                    Err(_) => todo!(),
                };
                info!("{:?}", symbol_info)
                // let symbol_info = serde_json::from_value::<SymbolInfo>(message[2].clone())?;
            }
            TradingViewDataEvent::OnReplayOk => {
                info!("replay ok: {:?}", message);
            }
            TradingViewDataEvent::OnReplayPoint => {
                info!("replay point: {:?}", message);
            }
            TradingViewDataEvent::OnReplayInstanceId => todo!("7"),
            TradingViewDataEvent::OnReplayResolutions => todo!("8"),
            TradingViewDataEvent::OnReplayDataEnd => todo!("9"),
            TradingViewDataEvent::OnStudyLoading => todo!("10"),
            TradingViewDataEvent::OnStudyCompleted => {
                info!("study completed: {:?}", message);
            }
            TradingViewDataEvent::OnError(_) => todo!("12"),
            TradingViewDataEvent::UnknownEvent(_) => todo!("13"),
        }
    }

    async fn handle_chart_data(
        &self,
        series: &HashMap<String, SeriesInfo>,
        studies: &HashMap<String, String>,
        message: &Vec<Value>
    ) -> Result<()> {
        for (id, s) in series.into_iter() {
            trace!("received v: {:?}, m: {:?}", s, message);
            match message[1].get(id.as_str()) {
                Some(resp_data) => {
                    let data: Vec<Vec<f64>> = ChartResponseData::deserialize(resp_data)?
                        .series.into_iter()
                        .map(|point| point.value)
                        .collect();
                    // timestamp, open, high, low, close, volume
                    debug!("series data extracted: {:?}", data);

                    // TODO: Notify function
                }
                None => {
                    debug!("receive empty data on series: {:?}", s);
                }
            }
        }
        for (k, v) in studies.into_iter() {
            if let Some(resp_data) = message[1].get(v.as_str()) {
                debug!("study data received: {} - {:?}", k, resp_data);
                let data: Vec<Vec<f64>> = StudyResponseData::deserialize(resp_data)?
                    .studies.into_iter()
                    .map(|point| point.value)
                    .collect();
                warn!("study data extracted: {} - {:?}", k, data);

                // TODO: Notify function
            }
        }

        Ok(())
    }

    async fn handle_study_data(&self) {}

    async fn handle_quote_data(&mut self, message: &Vec<Value>) {
        let qsd = QuoteData::deserialize(&message[1]).unwrap();
        if qsd.status == "ok" {
            if let Some(prev_quote) = self.metadata.quotes.get_mut(&qsd.name) {
                *prev_quote = merge_quotes(prev_quote, &qsd.value);
            } else {
                self.metadata.quotes.insert(qsd.name, qsd.value);
            }

            for (_, q) in &self.metadata.quotes {
                debug!("quote data: {:?}", q);
            }
        } else {
            error!("quote data status error: {:?}", qsd);
        }
    }
}
