use crate::{
    chart::models::{SeriesCompletedMessage, StudyResponseData},
    quote::models::QuoteData,
};
use futures_util::{future::BoxFuture, Future};
use serde_json::Value;
use std::sync::Arc;

pub type AsyncCallback<'a, T> = Box<dyn (Fn(T) -> BoxFuture<'a, ()>) + Send + Sync + 'a>;

#[derive(Clone)]
pub struct Callbacks<'a> {
    pub(crate) on_chart_data: Arc<AsyncCallback<'a, Vec<Vec<f64>>>>,
    pub(crate) on_quote_data: Arc<AsyncCallback<'a, QuoteData>>,
    pub(crate) on_series_completed: Arc<AsyncCallback<'a, SeriesCompletedMessage>>,
    pub(crate) on_study_completed: Arc<AsyncCallback<'a, StudyResponseData>>,
    pub(crate) on_unknown_event: Arc<AsyncCallback<'a, Value>>,
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
