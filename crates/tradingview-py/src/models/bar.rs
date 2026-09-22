use pyo3::exceptions::PyIndexError;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use serde::{Deserialize, Serialize};

use crate::PyObject;
use crate::models::enums::Interval;

/// Individual historical OHLCV price bar.
#[pyclass(name = "Bar", eq, from_py_object)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bar {
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
impl Bar {
    #[new]
    pub fn new(timestamp: i64, open: f64, high: f64, low: f64, close: f64, volume: f64) -> Self {
        Self {
            timestamp,
            open,
            high,
            low,
            close,
            volume,
        }
    }

    /// Return bar values as a Python dictionary.
    pub fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let dict = PyDict::new(py);
        dict.set_item("timestamp", self.timestamp)?;
        dict.set_item("open", self.open)?;
        dict.set_item("high", self.high)?;
        dict.set_item("low", self.low)?;
        dict.set_item("close", self.close)?;
        dict.set_item("volume", self.volume)?;
        Ok(dict)
    }

    /// Return bar values as a tuple: (timestamp, open, high, low, close, volume).
    pub fn to_tuple(&self) -> (i64, f64, f64, f64, f64, f64) {
        (
            self.timestamp,
            self.open,
            self.high,
            self.low,
            self.close,
            self.volume,
        )
    }

    fn __repr__(&self) -> String {
        format!(
            "Bar(timestamp={}, open={}, high={}, low={}, close={}, volume={})",
            self.timestamp, self.open, self.high, self.low, self.close, self.volume
        )
    }
}

/// Ordered collection of historical price bars for a symbol.
#[pyclass(name = "HistoricalSeries", from_py_object)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoricalSeries {
    #[pyo3(get)]
    pub symbol: String,
    #[pyo3(get)]
    pub exchange: String,
    #[pyo3(get)]
    pub interval: Interval,
    #[pyo3(get)]
    pub bars: Vec<Bar>,
}

#[pymethods]
impl HistoricalSeries {
    #[new]
    pub fn new(symbol: String, exchange: String, interval: Interval, bars: Vec<Bar>) -> Self {
        Self {
            symbol,
            exchange,
            interval,
            bars,
        }
    }

    pub fn __len__(&self) -> usize {
        self.bars.len()
    }

    pub fn __getitem__(&self, mut index: isize) -> PyResult<Bar> {
        let len = self.bars.len() as isize;
        if index < 0 {
            index += len;
        }
        if index < 0 || index >= len {
            return Err(PyIndexError::new_err("HistoricalSeries index out of range"));
        }
        Ok(self.bars[index as usize].clone())
    }

    pub fn __iter__(slf: PyRef<'_, Self>) -> PyResult<Py<HistoricalSeriesIter>> {
        let iter = HistoricalSeriesIter {
            bars: slf.bars.clone(),
            index: 0,
        };
        Py::new(slf.py(), iter)
    }

    /// Convert series to a column-oriented dictionary suitable for dataframe construction.
    pub fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let dict = PyDict::new(py);
        let timestamps: Vec<i64> = self.bars.iter().map(|b| b.timestamp).collect();
        let opens: Vec<f64> = self.bars.iter().map(|b| b.open).collect();
        let highs: Vec<f64> = self.bars.iter().map(|b| b.high).collect();
        let lows: Vec<f64> = self.bars.iter().map(|b| b.low).collect();
        let closes: Vec<f64> = self.bars.iter().map(|b| b.close).collect();
        let volumes: Vec<f64> = self.bars.iter().map(|b| b.volume).collect();

        dict.set_item("timestamp", timestamps)?;
        dict.set_item("open", opens)?;
        dict.set_item("high", highs)?;
        dict.set_item("low", lows)?;
        dict.set_item("close", closes)?;
        dict.set_item("volume", volumes)?;
        Ok(dict)
    }

    /// Convert series into a Polars DataFrame (requires polars).
    pub fn to_polars(&self, py: Python<'_>) -> PyResult<PyObject> {
        let polars = py.import("polars")?;
        let dict = self.to_dict(py)?;
        let df = polars.getattr("DataFrame")?.call1((dict,))?;
        Ok(df.unbind())
    }

    /// Convert series into a Pandas DataFrame (requires pandas).
    pub fn to_pandas(&self, py: Python<'_>) -> PyResult<PyObject> {
        let pandas = py.import("pandas")?;
        let dict = self.to_dict(py)?;
        let df = pandas.getattr("DataFrame")?.call1((dict,))?;
        Ok(df.unbind())
    }

    fn __repr__(&self) -> String {
        format!(
            "<HistoricalSeries: {} on {} ({} bars, interval={})>",
            self.symbol,
            self.exchange,
            self.bars.len(),
            self.interval.as_str()
        )
    }
}

/// Iterator over bars in a HistoricalSeries.
#[pyclass]
pub struct HistoricalSeriesIter {
    bars: Vec<Bar>,
    index: usize,
}

#[pymethods]
impl HistoricalSeriesIter {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&mut self) -> Option<Bar> {
        if self.index < self.bars.len() {
            let item = self.bars[self.index].clone();
            self.index += 1;
            Some(item)
        } else {
            None
        }
    }
}
