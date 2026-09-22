use pyo3::exceptions::PyIndexError;
use pyo3::prelude::*;
use pyo3::types::PyDict;
use serde::{Deserialize, Serialize};

use crate::PyObject;
use crate::models::enums::FinancialPeriod;

/// Individual corporate fundamental data point.
#[pyclass(name = "FundamentalPoint", eq, from_py_object)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FundamentalPoint {
    #[pyo3(get)]
    pub timestamp: i64,
    #[pyo3(get)]
    pub value: f64,
    #[pyo3(get)]
    pub index: i32,
}

#[pymethods]
impl FundamentalPoint {
    #[new]
    pub fn new(timestamp: i64, value: f64, index: i32) -> Self {
        Self {
            timestamp,
            value,
            index,
        }
    }

    /// Return point values as a dictionary.
    pub fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let dict = PyDict::new(py);
        dict.set_item("timestamp", self.timestamp)?;
        dict.set_item("value", self.value)?;
        dict.set_item("index", self.index)?;
        Ok(dict)
    }

    fn __repr__(&self) -> String {
        format!(
            "FundamentalPoint(timestamp={}, value={}, index={})",
            self.timestamp, self.value, self.index
        )
    }
}

/// Time series of corporate fundamental metric points.
#[pyclass(name = "FundamentalSeries", from_py_object)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FundamentalSeries {
    #[pyo3(get)]
    pub symbol: String,
    #[pyo3(get)]
    pub exchange: String,
    #[pyo3(get)]
    pub fund_id: String,
    #[pyo3(get)]
    pub period: FinancialPeriod,
    #[pyo3(get)]
    pub points: Vec<FundamentalPoint>,
}

#[pymethods]
impl FundamentalSeries {
    #[new]
    pub fn new(
        symbol: String,
        exchange: String,
        fund_id: String,
        period: FinancialPeriod,
        points: Vec<FundamentalPoint>,
    ) -> Self {
        Self {
            symbol,
            exchange,
            fund_id,
            period,
            points,
        }
    }

    pub fn __len__(&self) -> usize {
        self.points.len()
    }

    pub fn __getitem__(&self, mut index: isize) -> PyResult<FundamentalPoint> {
        let len = self.points.len() as isize;
        if index < 0 {
            index += len;
        }
        if index < 0 || index >= len {
            return Err(PyIndexError::new_err(
                "FundamentalSeries index out of range",
            ));
        }
        Ok(self.points[index as usize].clone())
    }

    pub fn __iter__(slf: PyRef<'_, Self>) -> PyResult<Py<FundamentalSeriesIter>> {
        let iter = FundamentalSeriesIter {
            points: slf.points.clone(),
            index: 0,
        };
        Py::new(slf.py(), iter)
    }

    /// Convert fundamental series to a dictionary.
    pub fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let dict = PyDict::new(py);
        dict.set_item("symbol", &self.symbol)?;
        dict.set_item("exchange", &self.exchange)?;
        dict.set_item("fund_id", &self.fund_id)?;
        dict.set_item("period", self.period.value())?;

        let timestamps: Vec<i64> = self.points.iter().map(|p| p.timestamp).collect();
        let values: Vec<f64> = self.points.iter().map(|p| p.value).collect();
        let indices: Vec<i32> = self.points.iter().map(|p| p.index).collect();

        dict.set_item("timestamp", timestamps)?;
        dict.set_item("value", values)?;
        dict.set_item("index", indices)?;
        Ok(dict)
    }

    /// Convert fundamental series into a Polars DataFrame (requires polars).
    pub fn to_polars(&self, py: Python<'_>) -> PyResult<PyObject> {
        let polars = py.import("polars")?;
        let dict = self.to_dict(py)?;
        let df = polars.getattr("DataFrame")?.call1((dict,))?;
        Ok(df.unbind())
    }

    /// Convert fundamental series into a Pandas DataFrame (requires pandas).
    pub fn to_pandas(&self, py: Python<'_>) -> PyResult<PyObject> {
        let pandas = py.import("pandas")?;
        let dict = self.to_dict(py)?;
        let df = pandas.getattr("DataFrame")?.call1((dict,))?;
        Ok(df.unbind())
    }

    fn __repr__(&self) -> String {
        format!(
            "<FundamentalSeries: {} {} ({}, {} points)>",
            self.symbol,
            self.fund_id,
            self.period.value(),
            self.points.len()
        )
    }
}

/// Iterator over points in a FundamentalSeries.
#[pyclass]
pub struct FundamentalSeriesIter {
    points: Vec<FundamentalPoint>,
    index: usize,
}

#[pymethods]
impl FundamentalSeriesIter {
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __next__(&mut self) -> Option<FundamentalPoint> {
        if self.index < self.points.len() {
            let item = self.points[self.index].clone();
            self.index += 1;
            Some(item)
        } else {
            None
        }
    }
}
