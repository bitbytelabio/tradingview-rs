use std::collections::HashMap;
use std::default;
use std::sync::Arc;

use serde::Deserialize;
use serde_json::Value;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::trace;

use crate::chart::models::{
    SymbolInfo,
    ChartResponseData,
    SeriesCompletedMessage,
    StudyResponseData,
};
use crate::chart::utils::{ extract_studies_data, extract_ohlcv_data };
use crate::chart::utils::get_string_value;
use crate::quote::models::{ QuoteData, QuoteValue };
use crate::socket::SocketMessageDe;
use crate::socket::SocketSession;
use crate::socket::TradingViewDataEvent;
use crate::socket::Socket;
use crate::Result;
use crate::chart::session::SeriesInfo;

#[derive(Default)]
pub struct Subscriber;

#[derive(Default)]
pub struct Metadata {
    pub series: HashMap<String, SeriesInfo>,
    pub studies: HashMap<String, String>,
    pub quote: HashMap<String, QuoteValue>,
}

impl Subscriber {
    // pub async fn subscribe<T: Socket + Send + Sync>(
    //     &mut self,
    //     listener: &mut T,
    //     socket: &mut SocketSession
    // ) {
    //     listener.event_loop(&mut socket.clone()).await;
    // }

    pub async fn event_handler(
        &self,
        event: TradingViewDataEvent,
        message: &Vec<Value>,
        metadata: Option<Metadata>
    ) {
        match event {
            TradingViewDataEvent::OnChartData | TradingViewDataEvent::OnChartDataUpdate => {
                trace!("received chart data: {:?}", message);
                if let Some(metadata) = metadata {
                    match
                        self.chart_data_handler(&metadata.series, &metadata.studies, message).await
                    {
                        Ok(_) => (),
                        Err(e) => error!("{}", e),
                    };
                }
            }
            TradingViewDataEvent::OnQuoteData => {
                let qsd = QuoteData::deserialize(&message[1]).unwrap();
            }
            TradingViewDataEvent::OnQuoteCompleted => todo!("2"),
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
            TradingViewDataEvent::OnReplayOk => todo!("5"),
            TradingViewDataEvent::OnReplayPoint => todo!("6"),
            TradingViewDataEvent::OnReplayInstanceId => todo!("7"),
            TradingViewDataEvent::OnReplayResolutions => todo!("8"),
            TradingViewDataEvent::OnReplayDataEnd => todo!("9"),
            TradingViewDataEvent::OnStudyLoading => todo!("10"),
            TradingViewDataEvent::OnStudyCompleted => todo!("11"),
            TradingViewDataEvent::OnError(_) => todo!("12"),
            TradingViewDataEvent::UnknownEvent(_) => todo!("13"),
        }
    }

    pub fn unsubscribe<T: Socket>(&mut self, event_type: TradingViewDataEvent, listener: T) {}

    pub fn notify(&self, event_type: TradingViewDataEvent) {}

    async fn chart_data_handler(
        &self,
        series: &HashMap<String, SeriesInfo>,
        studies: &HashMap<String, String>,
        message: &Vec<Value>
    ) -> Result<()> {
        for (id, s) in series.into_iter() {
            trace!("received v: {:?}, m: {:?}", s, message);
            match message[1].get(id.as_str()) {
                Some(resp_data) => {
                    let data = extract_ohlcv_data(ChartResponseData::deserialize(resp_data)?);
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
                let data = extract_studies_data(StudyResponseData::deserialize(resp_data)?);
                debug!("study data extracted: {} - {:?}", k, data);

                // TODO: Notify function
            }
        }

        Ok(())
    }

    async fn quote_data_handler() {}
}
