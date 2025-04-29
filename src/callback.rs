use crate::{
    Error,
    chart::{
        ChartOptions, StudyOptions,
        models::{DataPoint, StudyResponseData, SymbolInfo},
    },
    quote::models::QuoteValue,
};
use serde_json::Value;
use std::sync::Arc;
use tracing::info;

pub type Callback<T> = Box<dyn (Fn(T)) + Send + Sync>;

#[derive(Clone)]
pub struct Callbacks {
    pub on_symbol_info: Arc<Callback<SymbolInfo>>,

    pub on_series_loading: Arc<Callback<Vec<Value>>>,
    pub on_chart_data: Arc<Callback<(ChartOptions, Vec<DataPoint>)>>,
    pub on_series_completed: Arc<Callback<Vec<Value>>>,

    pub on_study_loading: Arc<Callback<Vec<Value>>>,
    pub on_study_data: Arc<Callback<(StudyOptions, StudyResponseData)>>,
    pub on_study_completed: Arc<Callback<Vec<Value>>>,

    pub on_quote_data: Arc<Callback<QuoteValue>>,
    pub on_quote_completed: Arc<Callback<Vec<Value>>>,

    pub on_replay_ok: Arc<Callback<Vec<Value>>>,
    pub on_replay_point: Arc<Callback<Vec<Value>>>,
    pub on_replay_instance_id: Arc<Callback<Vec<Value>>>,
    pub on_replay_resolutions: Arc<Callback<Vec<Value>>>,
    pub on_replay_data_end: Arc<Callback<Vec<Value>>>,

    pub on_error: Arc<Callback<(Error, Vec<Value>)>>,
    pub on_unknown_event: Arc<Callback<(String, Vec<Value>)>>,
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

            on_series_completed: Arc::new(Box::new(|data| {
                info!("callback trigger on series completed: {:?}", data);
            })),
            on_series_loading: Arc::new(Box::new(|data| {
                info!("callback trigger on series loading: {:?}", data);
            })),
            on_quote_completed: Arc::new(Box::new(|data| {
                info!("callback trigger on quote completed: {:?}", data);
            })),
            on_replay_ok: Arc::new(Box::new(|data| {
                info!("callback trigger on replay ok: {:?}", data);
            })),
            on_replay_point: Arc::new(Box::new(|data| {
                info!("callback trigger on replay point: {:?}", data);
            })),
            on_replay_instance_id: Arc::new(Box::new(|data| {
                info!("callback trigger on replay instance id: {:?}", data);
            })),
            on_replay_resolutions: Arc::new(Box::new(|data| {
                info!("callback trigger on replay resolutions: {:?}", data);
            })),
            on_replay_data_end: Arc::new(Box::new(|data| {
                info!("callback trigger on replay data end: {:?}", data);
            })),
            on_study_loading: Arc::new(Box::new(|data| {
                info!("callback trigger on study loading: {:?}", data);
            })),
            on_study_completed: Arc::new(Box::new(|data| {
                info!("callback trigger on study completed: {:?}", data);
            })),
            on_unknown_event: Arc::new(Box::new(|data| {
                info!("callback trigger on unknown event: {:?}", data);
            })),
        }
    }
}

impl Callbacks {
    pub fn on_chart_data(
        mut self,
        f: impl Fn((ChartOptions, Vec<DataPoint>)) + Send + Sync + 'static,
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

    pub fn on_error(
        mut self,
        f: impl Fn((Error, Vec<Value>)) + Send + Sync + 'static,
    ) -> Self {
        self.on_error = Arc::new(Box::new(f));
        self
    }

    pub fn on_symbol_info(mut self, f: impl Fn(SymbolInfo) + Send + Sync + 'static) -> Self {
        self.on_symbol_info = Arc::new(Box::new(f));
        self
    }

    pub fn on_series_completed(
        mut self,
        f: impl Fn(Vec<Value>) + Send + Sync + 'static,
    ) -> Self {
        self.on_series_completed = Arc::new(Box::new(f));
        self
    }

    pub fn on_series_loading(
        mut self,
        f: impl Fn(Vec<Value>) + Send + Sync + 'static,
    ) -> Self {
        self.on_series_loading = Arc::new(Box::new(f));
        self
    }

    pub fn on_quote_completed(
        mut self,
        f: impl Fn(Vec<Value>) + Send + Sync + 'static,
    ) -> Self {
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

    pub fn on_replay_instance_id(
        mut self,
        f: impl Fn(Vec<Value>) + Send + Sync + 'static,
    ) -> Self {
        self.on_replay_instance_id = Arc::new(Box::new(f));
        self
    }

    pub fn on_replay_resolutions(
        mut self,
        f: impl Fn(Vec<Value>) + Send + Sync + 'static,
    ) -> Self {
        self.on_replay_resolutions = Arc::new(Box::new(f));
        self
    }

    pub fn on_replay_data_end(
        mut self,
        f: impl Fn(Vec<Value>) + Send + Sync + 'static,
    ) -> Self {
        self.on_replay_data_end = Arc::new(Box::new(f));
        self
    }

    pub fn on_study_loading(
        mut self,
        f: impl Fn(Vec<Value>) + Send + Sync + 'static,
    ) -> Self {
        self.on_study_loading = Arc::new(Box::new(f));
        self
    }

    pub fn on_study_completed(
        mut self,
        f: impl Fn(Vec<Value>) + Send + Sync + 'static,
    ) -> Self {
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
