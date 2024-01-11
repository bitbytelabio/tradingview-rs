use crate::{
    chart::{
        models::{DataPoint, SeriesCompletedMessage, StudyResponseData},
        ChartOptions, StudyOptions,
    },
    quote::models::QuoteValue,
    Error,
};
use futures_util::{future::BoxFuture, Future};
use serde_json::Value;
use std::sync::Arc;
use tracing::error;

pub type AsyncCallback<'a, T> = Box<dyn (Fn(T) -> BoxFuture<'a, ()>) + Send + Sync + 'a>;

#[derive(Clone)]
pub struct Callbacks<'a> {
    pub(crate) on_chart_data: Arc<AsyncCallback<'a, (ChartOptions, Vec<DataPoint>)>>,
    pub(crate) on_quote_data: Arc<AsyncCallback<'a, QuoteValue>>,
    pub(crate) on_series_completed: Arc<AsyncCallback<'a, SeriesCompletedMessage>>,
    pub(crate) on_study_completed: Arc<AsyncCallback<'a, (StudyOptions, StudyResponseData)>>,
    pub(crate) on_unknown_event: Arc<AsyncCallback<'a, Value>>,
    pub(crate) on_error: Arc<AsyncCallback<'a, Error>>,
}

impl Default for Callbacks<'_> {
    fn default() -> Self {
        Self {
            on_chart_data: Arc::new(Box::new(|_| Box::pin(async {}))),
            on_quote_data: Arc::new(Box::new(|_| Box::pin(async {}))),
            on_series_completed: Arc::new(Box::new(|_| Box::pin(async {}))),
            on_study_completed: Arc::new(Box::new(|_| Box::pin(async {}))),
            on_unknown_event: Arc::new(Box::new(|_| Box::pin(async {}))),
            on_error: Arc::new(Box::new(|e| {
                Box::pin(
                    async move { error!("default error callback logging && handling, error: {e}") },
                )
            })),
        }
    }
}

impl<'a> Callbacks<'a> {
    pub fn on_chart_data<Fut>(
        &mut self,
        f: impl Fn((ChartOptions, Vec<DataPoint>)) -> Fut + Send + Sync + 'a,
    ) -> &mut Self
    where
        Fut: Future<Output = ()> + Send + 'a,
    {
        self.on_chart_data = Arc::new(Box::new(move |data| Box::pin(f(data))));
        self
    }

    pub fn on_quote_data<Fut>(
        &mut self,
        f: impl Fn(QuoteValue) -> Fut + Send + Sync + 'a,
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
        f: impl Fn((StudyOptions, StudyResponseData)) -> Fut + Send + Sync + 'a,
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

    pub fn on_error<Fut>(&mut self, f: impl Fn(Error) -> Fut + Send + Sync + 'a) -> &mut Self
    where
        Fut: Future<Output = ()> + Send + 'a,
    {
        self.on_error = Arc::new(Box::new(move |data| Box::pin(f(data))));
        self
    }
}
