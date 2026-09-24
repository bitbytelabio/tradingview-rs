pub mod callbacks;
pub mod client;
pub mod errors;
pub mod models;
pub mod streaming;

pub type PyObject = pyo3::Py<pyo3::PyAny>;

use pyo3::prelude::*;

#[pymodule]
fn _tradingview(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    // Register exceptions
    errors::register_exceptions(py, m)?;

    // Register enums
    m.add_class::<models::enums::Interval>()?;
    m.add_class::<models::enums::FinancialPeriod>()?;
    m.add_class::<models::enums::EconomicImportance>()?;
    m.add_class::<models::enums::DataServer>()?;

    // Register historical models
    m.add_class::<models::bar::Bar>()?;
    m.add_class::<models::bar::HistoricalSeries>()?;

    // Register streaming models
    m.add_class::<models::quote::QuoteTick>()?;
    m.add_class::<models::quote::QuoteSubscription>()?;
    m.add_class::<models::candle::CandleUpdate>()?;
    m.add_class::<models::candle::BarSubscription>()?;

    // Register fundamental & calendar models
    m.add_class::<models::fundamental::FundamentalPoint>()?;
    m.add_class::<models::fundamental::FundamentalSeries>()?;
    m.add_class::<models::calendar::EconomicEvent>()?;

    // Register client
    m.add_class::<client::TradingViewClient>()?;

    Ok(())
}
