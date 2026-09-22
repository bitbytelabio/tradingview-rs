use parking_lot::Mutex;
use pyo3::exceptions::PyStopAsyncIteration;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use pyo3_async_runtimes::tokio::future_into_py;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::mpsc::Receiver;

use crate::PyObject;
use crate::callbacks::CallbackDispatcher;
use crate::models::bar::Bar;
use crate::models::enums::Interval;

/// Real-time in-flight candlestick update or closed bar update.
#[pyclass(name = "CandleUpdate", eq, from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct CandleUpdate {
    #[pyo3(get)]
    pub symbol: String,
    #[pyo3(get)]
    pub interval: Interval,
    #[pyo3(get)]
    pub timestamp: i64,
    #[pyo3(get)]
    pub open: f64,
    #[pyo3(get)]
    pub high: f64,
    #[pyo3(get)]
    pub low: f64,
    #[pyo3(get)]
    pub close: f64,
    #[pyo3(get)]
    pub volume: f64,
}

#[pymethods]
impl CandleUpdate {
    #[new]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        symbol: String,
        interval: Interval,
        timestamp: i64,
        open: f64,
        high: f64,
        low: f64,
        close: f64,
        volume: f64,
    ) -> Self {
        Self {
            symbol,
            interval,
            timestamp,
            open,
            high,
            low,
            close,
            volume,
        }
    }

    /// Return candle update as a dictionary.
    pub fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let dict = PyDict::new(py);
        dict.set_item("symbol", &self.symbol)?;
        dict.set_item("interval", self.interval.value())?;
        dict.set_item("timestamp", self.timestamp)?;
        dict.set_item("open", self.open)?;
        dict.set_item("high", self.high)?;
        dict.set_item("low", self.low)?;
        dict.set_item("close", self.close)?;
        dict.set_item("volume", self.volume)?;
        Ok(dict)
    }

    /// Convert snapshot into a Bar instance.
    pub fn to_bar(&self) -> Bar {
        Bar {
            timestamp: self.timestamp,
            open: self.open,
            high: self.high,
            low: self.low,
            close: self.close,
            volume: self.volume,
        }
    }

    fn __repr__(&self) -> String {
        format!(
            "CandleUpdate(symbol='{}', interval='{}', o={}, h={}, l={}, c={}, v={}, ts={})",
            self.symbol,
            self.interval.value(),
            self.open,
            self.high,
            self.low,
            self.close,
            self.volume,
            self.timestamp
        )
    }
}

/// Active real-time candlestick bar progress subscription.
#[pyclass(name = "BarSubscription", from_py_object)]
#[derive(Clone)]
pub struct BarSubscription {
    receiver: Arc<tokio::sync::Mutex<Receiver<CandleUpdate>>>,
    dispatcher: CallbackDispatcher,
    is_stopped: Arc<AtomicBool>,
    stop_signal: Arc<Mutex<Option<tokio::sync::oneshot::Sender<()>>>>,
}

impl BarSubscription {
    pub fn new(
        receiver: Receiver<CandleUpdate>,
        dispatcher: CallbackDispatcher,
        stop_signal: tokio::sync::oneshot::Sender<()>,
    ) -> Self {
        Self {
            receiver: Arc::new(tokio::sync::Mutex::new(receiver)),
            dispatcher,
            is_stopped: Arc::new(AtomicBool::new(false)),
            stop_signal: Arc::new(Mutex::new(Some(stop_signal))),
        }
    }
}

#[pymethods]
impl BarSubscription {
    pub fn __aiter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    pub fn __anext__<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        let rx_lock = Arc::clone(&self.receiver);
        let stopped = Arc::clone(&self.is_stopped);

        future_into_py(py, async move {
            if stopped.load(Ordering::Relaxed) {
                return Err(PyStopAsyncIteration::new_err(()));
            }

            let mut rx = rx_lock.lock().await;
            match rx.recv().await {
                Some(candle) => Ok(candle),
                None => Err(PyStopAsyncIteration::new_err(())),
            }
        })
    }

    /// Add a synchronous callback to be executed for each candle update.
    pub fn add_callback(&self, py: Python<'_>, callback: PyObject) -> PyResult<()> {
        self.dispatcher.add_callback(py, callback)
    }

    /// Stop the bar subscription and terminate background listener tasks.
    pub fn stop<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        self.is_stopped.store(true, Ordering::Relaxed);
        let sender = self.stop_signal.lock().take();

        future_into_py(py, async move {
            if let Some(s) = sender {
                let _ = s.send(());
            }
            Ok(())
        })
    }
}
