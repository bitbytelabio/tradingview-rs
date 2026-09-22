use pyo3::prelude::*;
use pyo3::types::PyDict;
use serde::{Deserialize, Serialize};

use crate::models::enums::EconomicImportance;

/// Scheduled global macroeconomic calendar release.
#[pyclass(name = "EconomicEvent", eq, from_py_object)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EconomicEvent {
    #[pyo3(get)]
    pub id: String,
    #[pyo3(get)]
    pub title: String,
    #[pyo3(get)]
    pub country: String,
    #[pyo3(get)]
    pub indicator: String,
    #[pyo3(get)]
    pub ticker: String,
    #[pyo3(get)]
    pub date: i64,
    #[pyo3(get)]
    pub importance: EconomicImportance,
    #[pyo3(get)]
    pub actual: Option<f64>,
    #[pyo3(get)]
    pub forecast: Option<f64>,
    #[pyo3(get)]
    pub previous: Option<f64>,
}

#[pymethods]
impl EconomicEvent {
    #[new]
    #[allow(clippy::too_many_arguments)]
    #[pyo3(signature = (id, title, country, indicator, ticker, date, importance, actual = None, forecast = None, previous = None))]
    pub fn new(
        id: String,
        title: String,
        country: String,
        indicator: String,
        ticker: String,
        date: i64,
        importance: EconomicImportance,
        actual: Option<f64>,
        forecast: Option<f64>,
        previous: Option<f64>,
    ) -> Self {
        Self {
            id,
            title,
            country,
            indicator,
            ticker,
            date,
            importance,
            actual,
            forecast,
            previous,
        }
    }

    /// Return economic event attributes as a dictionary.
    pub fn to_dict<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
        let dict = PyDict::new(py);
        dict.set_item("id", &self.id)?;
        dict.set_item("title", &self.title)?;
        dict.set_item("country", &self.country)?;
        dict.set_item("indicator", &self.indicator)?;
        dict.set_item("ticker", &self.ticker)?;
        dict.set_item("date", self.date)?;
        dict.set_item("importance", self.importance.value())?;
        dict.set_item("actual", self.actual)?;
        dict.set_item("forecast", self.forecast)?;
        dict.set_item("previous", self.previous)?;
        Ok(dict)
    }

    fn __repr__(&self) -> String {
        format!(
            "EconomicEvent(id='{}', title='{}', country='{}', date={}, importance={:?}, actual={:?})",
            self.id, self.title, self.country, self.date, self.importance, self.actual
        )
    }
}
