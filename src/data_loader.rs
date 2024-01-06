use crate::{
    chart::{
        models::{ChartResponseData, SeriesCompletedMessage, StudyResponseData, SymbolInfo},
        session::SeriesInfo,
    },
    quote::{
        models::{QuoteData, QuoteValue},
        utils::merge_quotes,
    },
    socket::{Socket, SocketMessageDe, SocketSession, TradingViewDataEvent},
    Error, Result,
};
use futures_util::future::BoxFuture;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::pin::Pin;
use std::{collections::HashMap, sync::Arc};
use std::{fmt::Debug, future::Future};
use tracing::{debug, error, info, trace, warn};

#[derive(Clone, Default)]
pub struct DataLoader<'a> {
    pub(crate) metadata: Metadata,
    callbacks: Callbacks<'a>,
}

type AsyncCallback<'a, T> = Box<dyn (Fn(T) -> BoxFuture<'a, ()>) + Send + Sync + 'a>;

#[derive(Clone)]
pub struct Callbacks<'a> {
    on_chart_data: Arc<AsyncCallback<'a, Vec<Vec<f64>>>>,
    on_quote_data: Arc<AsyncCallback<'a, QuoteData>>,
    on_series_completed: Arc<AsyncCallback<'a, SeriesCompletedMessage>>,
    on_study_completed: Arc<AsyncCallback<'a, StudyResponseData>>,
    on_unknown_event: Arc<AsyncCallback<'a, Value>>,
}

impl Default for Callbacks<'_> {
    fn default() -> Self {
        Self {
            on_chart_data: Arc::new(Box::new(|_| Box::pin(async {}))),
            on_quote_data: Arc::new(Box::new(|_| Box::pin(async {}))),
            on_series_completed: Arc::new(Box::new(|_| Box::pin(async {}))),
            on_study_completed: Arc::new(Box::new(|_| Box::pin(async {}))),
            on_unknown_event: Arc::new(Box::new(|_| Box::pin(async {}))),
        }
    }
}

impl<'a> Callbacks<'a> {
    pub fn on_chart_data<Fut>(
        &mut self,
        f: impl Fn(Vec<Vec<f64>>) -> Fut + Send + Sync + 'a,
    ) -> &mut Self
    where
        Fut: Future<Output = ()> + Send + 'a,
    {
        self.on_chart_data = Arc::new(Box::new(move |data| Box::pin(f(data))));
        self
    }

    pub fn on_quote_data<Fut>(
        &mut self,
        f: impl Fn(QuoteData) -> Fut + Send + Sync + 'a,
    ) -> &mut Self
    where
        Fut: Future<Output = ()> + Send + 'a,
    {
        self.on_quote_data = Arc::new(Box::new(move |data| Box::pin(f(data))));
        self
    }

    pub fn on_series_completed<Fut>(
        &mut self,
        f: impl Fn(SeriesCompletedMessage) -> Fut + Send + Sync + 'a,
    ) -> &mut Self
    where
        Fut: Future<Output = ()> + Send + 'a,
    {
        self.on_series_completed = Arc::new(Box::new(move |data| Box::pin(f(data))));
        self
    }

    pub fn on_study_completed<Fut>(
        &mut self,
        f: impl Fn(StudyResponseData) -> Fut + Send + Sync + 'a,
    ) -> &mut Self
    where
        Fut: Future<Output = ()> + Send + 'a,
    {
        self.on_study_completed = Arc::new(Box::new(move |data| Box::pin(f(data))));
        self
    }

    pub fn on_unknown_event<Fut>(
        &mut self,
        f: impl Fn(Value) -> Fut + Send + Sync + 'a,
    ) -> &mut Self
    where
        Fut: Future<Output = ()> + Send + 'a,
    {
        self.on_unknown_event = Arc::new(Box::new(move |data| Box::pin(f(data))));
        self
    }
}

pub struct FeederBuilder {}

impl FeederBuilder {
    pub fn new() -> Self {
        Self {}
    }
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

impl<'a> DataLoader<'a> {
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
            callbacks: Callbacks::default(),
            // publisher: Vec::new(),
        }
    }

    pub fn process_with_callback(
        &mut self,
        event: TradingViewDataEvent,
        callback: Box<AsyncCallback<'a, Box<dyn Send + Sync>>>,
    ) -> &mut Self {
        // let cb = Arc::new(callback);
        // self.callbacks.insert(event, cb);
        self
    }

    // pub fn add(mut self, publisher: Box<dyn Socket + Send + Sync>) -> Self {
    //     self.publisher.push(publisher);
    //     self
    // }

    // pub fn unsubscribe<T: Socket + Send>(&mut self, event_type: TradingViewDataEvent, listener: T) {
    //     todo!()
    // }

    // pub fn notify<T: serde::Serialize>(&self, event_type: TradingViewDataEvent, data: T) {
    //     todo!()
    // }

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
                    Err(e) => error!("{}", e),
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
        message: &Vec<Value>,
    ) -> Result<()> {
        for (id, s) in series.into_iter() {
            trace!("received v: {:?}, m: {:?}", s, message);
            match message[1].get(id.as_str()) {
                Some(resp_data) => {
                    let data: Vec<Vec<f64>> = ChartResponseData::deserialize(resp_data)?
                        .series
                        .into_iter()
                        .map(|point| point.value)
                        .collect();
                    // timestamp, open, high, low, close, volume
                    debug!("series data extracted: {:?}", data);
                    // self.notify(TradingViewDataEvent::OnChartData, data)

                    // TODO: Notify function
                    (self.callbacks.on_chart_data)(data).await;
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
                    .studies
                    .into_iter()
                    .map(|point| point.value)
                    .collect();
                warn!("study data extracted: {} - {:?}", k, data);

                // TODO: Notify function
            }
        }

        Ok(())
    }

    async fn handle_study_data(&self) {
        todo!()
    }

    async fn handle_quote_data(&mut self, message: &Vec<Value>) {
        let qsd = QuoteData::deserialize(&message[1]).unwrap();
        if qsd.status == "ok" {
            if let Some(prev_quote) = self.metadata.quotes.get_mut(&qsd.name) {
                *prev_quote = merge_quotes(prev_quote, &qsd.value);
            } else {
                self.metadata.quotes.insert(qsd.name, qsd.value);
            }

            for q in self.metadata.quotes.values() {
                debug!("quote data: {:?}", q);
                // TODO: Notify function for quote data
            }
        } else {
            error!("quote data status error: {:?}", qsd);
            // TODO: Notify function for quote data error
        }
    }
}
