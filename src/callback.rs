use crate::{
    Error,
    chart::{
        StudyOptions,
        models::{DataPoint, StudyResponseData, SymbolInfo},
    },
    quote::models::QuoteValue,
    websocket::SeriesInfo,
};
use bon::Builder;
use serde_json::Value;
use std::sync::Arc;

pub type Callback<T> = Box<dyn Fn(T) + Send + Sync + 'static>;

fn default_callback<T: std::fmt::Debug>(name: &'static str) -> Arc<Callback<T>> {
    Arc::new(Box::new(move |data| {
        tracing::trace!("Callback trigger on {}: {:?}", name, data);
    }))
}

#[derive(Clone, Builder)]
pub struct Callbacks {
    #[builder(default= default_callback::<SymbolInfo>("ON_SYMBOL_INFO"))]
    pub on_symbol_info: Arc<Callback<SymbolInfo>>,

    #[builder(default= default_callback::<Vec<Value>>("ON_SERIES_LOADING"))]
    pub on_series_loading: Arc<Callback<Vec<Value>>>,

    #[builder(default= default_callback::<(SeriesInfo, Vec<DataPoint>)>("ON_CHART_DATA"))]
    pub on_chart_data: Arc<Callback<(SeriesInfo, Vec<DataPoint>)>>,

    #[builder(default= default_callback::<Vec<Value>>("ON_SERIES_COMPLETED"))]
    pub on_series_completed: Arc<Callback<Vec<Value>>>,

    #[builder(default= default_callback::<Vec<Value>>("ON_STUDY_LOADING"))]
    pub on_study_loading: Arc<Callback<Vec<Value>>>,

    #[builder(default= default_callback::<(StudyOptions, StudyResponseData)>("ON_STUDY_DATA"))]
    pub on_study_data: Arc<Callback<(StudyOptions, StudyResponseData)>>,

    #[builder(default= default_callback::<Vec<Value>>("ON_STUDY_COMPLETED"))]
    pub on_study_completed: Arc<Callback<Vec<Value>>>,

    #[builder(default= default_callback::<QuoteValue>("ON_QUOTE_DATA"))]
    pub on_quote_data: Arc<Callback<QuoteValue>>,

    #[builder(default= default_callback::<Vec<Value>>("ON_QUOTE_COMPLETED"))]
    pub on_quote_completed: Arc<Callback<Vec<Value>>>,

    #[builder(default= default_callback::<Vec<Value>>("ON_REPLAY_OK"))]
    pub on_replay_ok: Arc<Callback<Vec<Value>>>,

    #[builder(default= default_callback::<Vec<Value>>("ON_REPLAY_POINT"))]
    pub on_replay_point: Arc<Callback<Vec<Value>>>,

    #[builder(default= default_callback::<Vec<Value>>("ON_REPLAY_INSTANCE_ID"))]
    pub on_replay_instance_id: Arc<Callback<Vec<Value>>>,

    #[builder(default= default_callback::<Vec<Value>>("ON_REPLAY_RESOLUTIONS"))]
    pub on_replay_resolutions: Arc<Callback<Vec<Value>>>,

    #[builder(default= default_callback::<Vec<Value>>("ON_REPLAY_DATA_END"))]
    pub on_replay_data_end: Arc<Callback<Vec<Value>>>,

    #[builder(default= default_callback::<(Error, Vec<Value>)>("ON_ERROR"))]
    pub on_error: Arc<Callback<(Error, Vec<Value>)>>,

    #[builder(default= default_callback::<(String, Vec<Value>)>("ON_UNKNOWN_EVENT"))]
    pub on_unknown_event: Arc<Callback<(String, Vec<Value>)>>,
}

impl Default for Callbacks {
    fn default() -> Self {
        Callbacks::builder().build()
    }
}

impl Callbacks {
    pub fn on_chart_data(
        mut self,
        f: impl Fn((SeriesInfo, Vec<DataPoint>)) + Send + Sync + 'static,
    ) -> Self {
        self.on_chart_data = Arc::new(Box::new(f));
        self
    }

    pub fn on_quote_data(mut self, f: impl Fn(QuoteValue) + Send + Sync + 'static) -> Self {
        self.on_quote_data = Arc::new(Box::new(f));
        self
    }

    pub fn on_study_data(
        mut self,
        f: impl Fn((StudyOptions, StudyResponseData)) + Send + Sync + 'static,
    ) -> Self {
        self.on_study_data = Arc::new(Box::new(f));
        self
    }

    pub fn on_error(mut self, f: impl Fn((Error, Vec<Value>)) + Send + Sync + 'static) -> Self {
        self.on_error = Arc::new(Box::new(f));
        self
    }

    pub fn on_symbol_info(mut self, f: impl Fn(SymbolInfo) + Send + Sync + 'static) -> Self {
        self.on_symbol_info = Arc::new(Box::new(f));
        self
    }

    pub fn on_series_completed(mut self, f: impl Fn(Vec<Value>) + Send + Sync + 'static) -> Self {
        self.on_series_completed = Arc::new(Box::new(f));
        self
    }

    pub fn on_series_loading(mut self, f: impl Fn(Vec<Value>) + Send + Sync + 'static) -> Self {
        self.on_series_loading = Arc::new(Box::new(f));
        self
    }

    pub fn on_quote_completed(mut self, f: impl Fn(Vec<Value>) + Send + Sync + 'static) -> Self {
        self.on_quote_completed = Arc::new(Box::new(f));
        self
    }

    pub fn on_replay_ok(mut self, f: impl Fn(Vec<Value>) + Send + Sync + 'static) -> Self {
        self.on_replay_ok = Arc::new(Box::new(f));
        self
    }

    pub fn on_replay_point(mut self, f: impl Fn(Vec<Value>) + Send + Sync + 'static) -> Self {
        self.on_replay_point = Arc::new(Box::new(f));
        self
    }

    pub fn on_replay_instance_id(mut self, f: impl Fn(Vec<Value>) + Send + Sync + 'static) -> Self {
        self.on_replay_instance_id = Arc::new(Box::new(f));
        self
    }

    pub fn on_replay_resolutions(mut self, f: impl Fn(Vec<Value>) + Send + Sync + 'static) -> Self {
        self.on_replay_resolutions = Arc::new(Box::new(f));
        self
    }

    pub fn on_replay_data_end(mut self, f: impl Fn(Vec<Value>) + Send + Sync + 'static) -> Self {
        self.on_replay_data_end = Arc::new(Box::new(f));
        self
    }

    pub fn on_study_loading(mut self, f: impl Fn(Vec<Value>) + Send + Sync + 'static) -> Self {
        self.on_study_loading = Arc::new(Box::new(f));
        self
    }

    pub fn on_study_completed(mut self, f: impl Fn(Vec<Value>) + Send + Sync + 'static) -> Self {
        self.on_study_completed = Arc::new(Box::new(f));
        self
    }

    pub fn on_unknown_event(
        mut self,
        f: impl Fn((String, Vec<Value>)) + Send + Sync + 'static,
    ) -> Self {
        self.on_unknown_event = Arc::new(Box::new(f));
        self
    }
}
