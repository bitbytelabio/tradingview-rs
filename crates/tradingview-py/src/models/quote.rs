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

/// Real-time market quote tick update.
#[pyclass(name = "QuoteTick", eq, from_py_object)]
#[derive(Debug, Clone, PartialEq)]
pub struct QuoteTick {
    #[pyo3(get)]
    pub symbol: String,
    #[pyo3(get)]
    pub timestamp: i64,
    #[pyo3(get)]
    pub price: f64,
    #[pyo3(get)]
    pub volume: f64,
    #[pyo3(get)]
    pub bid: Option<f64>,
    #[pyo3(get)]
    pub ask: Option<f64>,
    #[pyo3(get)]
    pub change: Option<f64>,
    #[pyo3(get)]
    pub change_percent: Option<f64>,
}

#[pymethods]
impl QuoteTick {
    #[new]
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (symbol, timestamp, price, volume, bid = None, ask = None, change = None, change_percent = None))]
    pub fn new(
        symbol: String,
        timestamp: i64,
        price: f64,
        volume: f64,
        bid: Option<f64>,
        ask: Option<f64>,
        change: Option<f64>,
        change_percent: Option<f64>,
    ) -> Self {
        Self {
            symbol,
            timestamp,
            price,
            volume,
            bid,
            ask,
            change,
            change_percent,
        }
    }

    /// Return tick as a dictionary.
    pub fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let dict = PyDict::new(py);
        dict.set_item("symbol", &self.symbol)?;
        dict.set_item("timestamp", self.timestamp)?;
        dict.set_item("price", self.price)?;
        dict.set_item("volume", self.volume)?;
        dict.set_item("bid", self.bid)?;
        dict.set_item("ask", self.ask)?;
        dict.set_item("change", self.change)?;
        dict.set_item("change_percent", self.change_percent)?;
        Ok(dict)
    }

    fn __repr__(&self) -> String {
        format!(
            "QuoteTick(symbol='{}', price={}, volume={}, bid={:?}, ask={:?}, ts={})",
            self.symbol, self.price, self.volume, self.bid, self.ask, self.timestamp
        )
    }
}

/// Active real-time market quote stream subscription.
#[pyclass(name = "QuoteSubscription", from_py_object)]
#[derive(Clone)]
pub struct QuoteSubscription {
    receiver: Arc<tokio::sync::Mutex<Receiver<QuoteTick>>>,
    dispatcher: CallbackDispatcher,
    is_stopped: Arc<AtomicBool>,
    stop_signal: Arc<Mutex<Option<tokio::sync::oneshot::Sender<()>>>>,
}

impl QuoteSubscription {
    pub fn new(
        receiver: Receiver<QuoteTick>,
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
impl QuoteSubscription {
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
                Some(tick) => Ok(tick),
                None => Err(PyStopAsyncIteration::new_err(())),
            }
        })
    }

    /// Add a synchronous callback to be executed for each quote tick.
    pub fn add_callback(&self, py: Python<'_>, callback: PyObject) -> PyResult<()> {
        self.dispatcher.add_callback(py, callback)
    }

    /// Stop the quote subscription and terminate background listener tasks.
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
