use crate::{
    Error,
    chart::{
        ChartOptions, StudyOptions,
        models::{DataPoint, StudyResponseData, SymbolInfo},
    },
    quote::models::QuoteValue,
    socket::TradingViewDataEvent,
};
use futures_util::{Future, future::BoxFuture};
use serde_json::Value;
use std::sync::Arc;
use tracing::{error, info};

pub type AsyncCallback<'a, T> = Box<dyn (Fn(T) -> BoxFuture<'a, ()>) + Send + Sync + 'a>;

#[derive(Clone)]
pub struct Callbacks<'a> {
    pub(crate) on_chart_data: Arc<AsyncCallback<'a, (ChartOptions, Vec<DataPoint>)>>,
    pub(crate) on_quote_data: Arc<AsyncCallback<'a, QuoteValue>>,
    pub(crate) on_study_data: Arc<AsyncCallback<'a, (StudyOptions, StudyResponseData)>>,
    pub(crate) on_error: Arc<AsyncCallback<'a, Error>>,
    pub(crate) on_symbol_info: Arc<AsyncCallback<'a, SymbolInfo>>,
    pub(crate) on_other_event: Arc<AsyncCallback<'a, (TradingViewDataEvent, Vec<Value>)>>,
}

impl Default for Callbacks<'_> {
    fn default() -> Self {
        Self {
            on_chart_data: Arc::new(Box::new(|data| {
                Box::pin(async move { info!("default callback logging && handling: {:?}", data) })
            })),
            on_quote_data: Arc::new(Box::new(|data| {
                Box::pin(async move { info!("default callback logging && handling: {:?}", data) })
            })),
            on_study_data: Arc::new(Box::new(|data| {
                Box::pin(async move { info!("default callback logging && handling: {:?}", data) })
            })),

            on_error: Arc::new(Box::new(|e| {
                Box::pin(async move { error!("default callback logging && handling: {e}") })
            })),
            on_symbol_info: Arc::new(Box::new(|data| {
                Box::pin(async move { info!("default callback logging && handling: {:?}", data) })
            })),
            on_other_event: Arc::new(Box::new(|data| {
                Box::pin(async move { info!("default callback logging && handling: {:?}", data) })
            })),
        }
    }
}

impl<'a> Callbacks<'a> {
    pub fn on_chart_data<Fut>(
        mut self,
        f: impl Fn((ChartOptions, Vec<DataPoint>)) -> Fut + Send + Sync + 'a,
    ) -> Self
    where
        Fut: Future<Output = ()> + Send + 'a,
    {
        self.on_chart_data = Arc::new(Box::new(move |data| Box::pin(f(data))));
        self
    }

    pub fn on_quote_data<Fut>(mut self, f: impl Fn(QuoteValue) -> Fut + Send + Sync + 'a) -> Self
    where
        Fut: Future<Output = ()> + Send + 'a,
    {
        self.on_quote_data = Arc::new(Box::new(move |data| Box::pin(f(data))));
        self
    }

    pub fn on_study_data<Fut>(
        mut self,
        f: impl Fn((StudyOptions, StudyResponseData)) -> Fut + Send + Sync + 'a,
    ) -> Self
    where
        Fut: Future<Output = ()> + Send + 'a,
    {
        self.on_study_data = Arc::new(Box::new(move |data| Box::pin(f(data))));
        self
    }

    pub fn on_error<Fut>(mut self, f: impl Fn(Error) -> Fut + Send + Sync + 'a) -> Self
    where
        Fut: Future<Output = ()> + Send + 'a,
    {
        self.on_error = Arc::new(Box::new(move |data| Box::pin(f(data))));
        self
    }

    pub fn on_symbol_info<Fut>(mut self, f: impl Fn(SymbolInfo) -> Fut + Send + Sync + 'a) -> Self
    where
        Fut: Future<Output = ()> + Send + 'a,
    {
        self.on_symbol_info = Arc::new(Box::new(move |data| Box::pin(f(data))));
        self
    }

    pub fn on_other_event<Fut>(
        mut self,
        f: impl Fn((TradingViewDataEvent, Vec<Value>)) -> Fut + Send + Sync + 'a,
    ) -> Self
    where
        Fut: Future<Output = ()> + Send + 'a,
    {
        self.on_other_event = Arc::new(Box::new(move |data| Box::pin(f(data))));
        self
    }
}
