use crate::{
    Error,
    chart::{
        ChartOptions, StudyOptions,
        models::{DataPoint, StudyResponseData, SymbolInfo},
    },
    quote::models::QuoteValue,
    socket::TradingViewDataEvent,
};
use serde_json::Value;
use std::future::Future;
use std::{pin::Pin, sync::Arc};
use tracing::info;

pub type AsyncCallback<'a, T> =
    Box<dyn (Fn(T) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>>) + Send + Sync + 'a>;

pub type Callback<T> = Box<dyn (Fn(T) -> ()) + Send + Sync>;

#[derive(Clone)]
pub struct Callbacks {
    pub on_chart_data: Arc<Callback<(ChartOptions, Vec<DataPoint>)>>,
    pub on_quote_data: Arc<Callback<QuoteValue>>,
    pub on_study_data: Arc<Callback<(StudyOptions, StudyResponseData)>>,
    pub on_error: Arc<Callback<Error>>,
    pub on_symbol_info: Arc<Callback<SymbolInfo>>,
    pub on_other_event: Arc<Callback<(TradingViewDataEvent, Vec<Value>)>>,
}

impl Default for Callbacks {
    fn default() -> Self {
        Self {
            on_chart_data: Arc::new(Box::new(|data| {
                info!("callback trigger on chart data: {:?}", data);
            })),
            on_quote_data: Arc::new(Box::new(|data| {
                info!("callback trigger on quote data: {:?}", data);
            })),
            on_study_data: Arc::new(Box::new(|data| {
                info!("callback trigger on study data: {:?}", data);
            })),

            on_error: Arc::new(Box::new(|data| {
                info!("callback trigger on error: {:?}", data);
            })),
            on_symbol_info: Arc::new(Box::new(|data| {
                info!("callback trigger on symbol info: {:?}", data);
            })),
            on_other_event: Arc::new(Box::new(|data| {
                info!("callback trigger on other event: {:?}", data);
            })),
        }
    }
}

impl Callbacks {
    pub fn on_chart_data(
        mut self,
        f: impl Fn((ChartOptions, Vec<DataPoint>)) -> () + Send + Sync + 'static,
    ) -> Self {
        self.on_chart_data = Arc::new(Box::new(f));
        self
    }

    pub fn on_quote_data(mut self, f: impl Fn(QuoteValue) -> () + Send + Sync + 'static) -> Self {
        self.on_quote_data = Arc::new(Box::new(f));
        self
    }

    pub fn on_study_data(
        mut self,
        f: impl Fn((StudyOptions, StudyResponseData)) -> () + Send + Sync + 'static,
    ) -> Self {
        self.on_study_data = Arc::new(Box::new(f));
        self
    }

    pub fn on_error(mut self, f: impl Fn(Error) -> () + Send + Sync + 'static) -> Self {
        self.on_error = Arc::new(Box::new(f));
        self
    }

    pub fn on_symbol_info(mut self, f: impl Fn(SymbolInfo) -> () + Send + Sync + 'static) -> Self {
        self.on_symbol_info = Arc::new(Box::new(f));
        self
    }

    pub fn on_other_event(
        mut self,
        f: impl Fn((TradingViewDataEvent, Vec<Value>)) -> () + Send + Sync + 'static,
    ) -> Self {
        self.on_other_event = Arc::new(Box::new(f));
        self
    }
}
