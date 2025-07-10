use crate::{
    Error,
    chart::{DataPoint, StudyOptions, StudyResponseData, SymbolInfo},
    live::handler::message::Command,
    quote::models::QuoteValue,
    websocket::SeriesInfo,
};
use bon::Builder;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use ustr::Ustr;

pub type CommandTx = UnboundedSender<Command>;
pub type CommandRx = UnboundedReceiver<Command>;

pub type CallbackFn<T> = Box<dyn Fn(T) + Send + Sync + 'static>;

fn default_callback<T: std::fmt::Debug>(name: &'static str) -> Arc<CallbackFn<T>> {
    Arc::new(Box::new(move |data| {
        tracing::trace!("Callback trigger on {}: {:?}", name, data);
    }))
}

// Macro to generate setter methods assuming method name and field name are the same
macro_rules! event_setter {
    ($name:ident, $param_type:ty) => {
        pub fn $name(mut self, f: impl Fn($param_type) + Send + Sync + 'static) -> Self {
            self.$name = Arc::new(Box::new(f));
            self
        }
    };
    // Variant for tupled parameters
    ($name:ident, ($($param_type_tuple:ty),+)) => {
        pub fn $name(mut self, f: impl Fn(($($param_type_tuple),+)) + Send + Sync + 'static) -> Self {
            self.$name = Arc::new(Box::new(f));
            self
        }
    };
}

#[derive(Clone, Builder)]
pub struct TradingViewEventHandler {
    #[builder(default= default_callback::<SymbolInfo>("ON_SYMBOL_INFO"))]
    pub on_symbol_info: Arc<CallbackFn<SymbolInfo>>,

    #[builder(default= default_callback::<Vec<Value>>("ON_SERIES_LOADING"))]
    pub on_series_loading: Arc<CallbackFn<Vec<Value>>>,

    #[builder(default= default_callback::<(SeriesInfo, Vec<DataPoint>)>("ON_CHART_DATA"))]
    pub on_chart_data: Arc<CallbackFn<(SeriesInfo, Vec<DataPoint>)>>,

    #[builder(default= default_callback::<Vec<Value>>("ON_SERIES_COMPLETED"))]
    pub on_series_completed: Arc<CallbackFn<Vec<Value>>>,

    #[builder(default= default_callback::<Vec<Value>>("ON_STUDY_LOADING"))]
    pub on_study_loading: Arc<CallbackFn<Vec<Value>>>,

    #[builder(default= default_callback::<(StudyOptions, StudyResponseData)>("ON_STUDY_DATA"))]
    pub on_study_data: Arc<CallbackFn<(StudyOptions, StudyResponseData)>>,

    #[builder(default= default_callback::<Vec<Value>>("ON_STUDY_COMPLETED"))]
    pub on_study_completed: Arc<CallbackFn<Vec<Value>>>,

    #[builder(default= default_callback::<QuoteValue>("ON_QUOTE_DATA"))]
    pub on_quote_data: Arc<CallbackFn<QuoteValue>>,

    #[builder(default= default_callback::<Vec<Value>>("ON_QUOTE_COMPLETED"))]
    pub on_quote_completed: Arc<CallbackFn<Vec<Value>>>,

    #[builder(default= default_callback::<Vec<Value>>("ON_REPLAY_OK"))]
    pub on_replay_ok: Arc<CallbackFn<Vec<Value>>>,

    #[builder(default= default_callback::<Vec<Value>>("ON_REPLAY_POINT"))]
    pub on_replay_point: Arc<CallbackFn<Vec<Value>>>,

    #[builder(default= default_callback::<Vec<Value>>("ON_REPLAY_INSTANCE_ID"))]
    pub on_replay_instance_id: Arc<CallbackFn<Vec<Value>>>,

    #[builder(default= default_callback::<Vec<Value>>("ON_REPLAY_RESOLUTIONS"))]
    pub on_replay_resolutions: Arc<CallbackFn<Vec<Value>>>,

    #[builder(default= default_callback::<Vec<Value>>("ON_REPLAY_DATA_END"))]
    pub on_replay_data_end: Arc<CallbackFn<Vec<Value>>>,

    #[builder(default= default_callback::<(Error, Vec<Value>)>("ON_ERROR"))]
    pub on_error: Arc<CallbackFn<(Error, Vec<Value>)>>,

    #[builder(default= default_callback::<(Ustr, Vec<Value>)>("ON_UNKNOWN_EVENT"))]
    pub on_unknown_event: Arc<CallbackFn<(Ustr, Vec<Value>)>>,
}

impl Default for TradingViewEventHandler {
    fn default() -> Self {
        TradingViewEventHandler::builder().build()
    }
}

impl TradingViewEventHandler {
    event_setter!(on_chart_data, (SeriesInfo, Vec<DataPoint>));
    event_setter!(on_quote_data, QuoteValue);
    event_setter!(on_study_data, (StudyOptions, StudyResponseData));
    event_setter!(on_error, (Error, Vec<Value>));
    event_setter!(on_symbol_info, SymbolInfo);
    event_setter!(on_series_completed, Vec<Value>);
    event_setter!(on_series_loading, Vec<Value>);
    event_setter!(on_quote_completed, Vec<Value>);
    event_setter!(on_replay_ok, Vec<Value>);
    event_setter!(on_replay_point, Vec<Value>);
    event_setter!(on_replay_instance_id, Vec<Value>);
    event_setter!(on_replay_resolutions, Vec<Value>);
    event_setter!(on_replay_data_end, Vec<Value>);
    event_setter!(on_study_loading, Vec<Value>);
    event_setter!(on_study_completed, Vec<Value>);
    event_setter!(on_unknown_event, (Ustr, Vec<Value>));
}
