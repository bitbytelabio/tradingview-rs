use bon::Builder;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::{
    Error, QuoteValue, SeriesDataResponse, TradingViewDataEvent, error::TradingViewError,
    live::handler::command::Command,
};

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
pub struct EventHandler {
    #[builder(default = default_callback::<(TradingViewDataEvent, Vec<SeriesDataResponse>)>("ON_DATA"))]
    pub on_series_data: Arc<CallbackFn<(TradingViewDataEvent, Vec<SeriesDataResponse>)>>,

    #[builder(default = default_callback::<(TradingViewDataEvent, Vec<Value>)>("ON_SIGNAL"))]
    pub on_signal: Arc<CallbackFn<(TradingViewDataEvent, Vec<Value>)>>,

    #[builder(default= default_callback::<(TradingViewError, Vec<Value>)>("ON_ERROR"))]
    pub on_tradingview_error: Arc<CallbackFn<(TradingViewError, Vec<Value>)>>,

    #[builder(default = default_callback::<(Error, Vec<Value>)>("ON_INTERNAL_ERROR"))]
    pub on_internal_error: Arc<CallbackFn<(Error, Vec<Value>)>>,

    #[builder(default = default_callback::<QuoteValue>("ON_QUOTE_DATA"))]
    pub on_quote_data: Arc<CallbackFn<QuoteValue>>,
}

impl EventHandler {
    event_setter!(on_internal_error, (Error, Vec<Value>));
    event_setter!(
        on_series_data,
        (TradingViewDataEvent, Vec<SeriesDataResponse>)
    );
    event_setter!(on_tradingview_error, (TradingViewError, Vec<Value>));
    event_setter!(on_signal, (TradingViewDataEvent, Vec<Value>));
}
