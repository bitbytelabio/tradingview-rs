use pyo3::prelude::*;
use serde::{Deserialize, Serialize};

/// Candlestick interval timeframe.
#[pyclass(eq, eq_int, from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Interval {
    OneMinute,
    ThreeMinutes,
    FiveMinutes,
    FifteenMinutes,
    ThirtyMinutes,
    FortyFiveMinutes,
    OneHour,
    TwoHours,
    ThreeHours,
    FourHours,
    OneDay,
    OneWeek,
    OneMonth,
}

#[pymethods]
impl Interval {
    #[getter]
    pub fn value(&self) -> &'static str {
        self.as_str()
    }

    #[getter]
    pub fn name(&self) -> &'static str {
        match self {
            Self::OneMinute => "OneMinute",
            Self::ThreeMinutes => "ThreeMinutes",
            Self::FiveMinutes => "FiveMinutes",
            Self::FifteenMinutes => "FifteenMinutes",
            Self::ThirtyMinutes => "ThirtyMinutes",
            Self::FortyFiveMinutes => "FortyFiveMinutes",
            Self::OneHour => "OneHour",
            Self::TwoHours => "TwoHours",
            Self::ThreeHours => "ThreeHours",
            Self::FourHours => "FourHours",
            Self::OneDay => "OneDay",
            Self::OneWeek => "OneWeek",
            Self::OneMonth => "OneMonth",
        }
    }

    fn __repr__(&self) -> String {
        format!("<Interval.{}: '{}'>", self.name(), self.value())
    }

    fn __str__(&self) -> &'static str {
        self.as_str()
    }
}

impl Interval {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::OneMinute => "1",
            Self::ThreeMinutes => "3",
            Self::FiveMinutes => "5",
            Self::FifteenMinutes => "15",
            Self::ThirtyMinutes => "30",
            Self::FortyFiveMinutes => "45",
            Self::OneHour => "60",
            Self::TwoHours => "120",
            Self::ThreeHours => "180",
            Self::FourHours => "240",
            Self::OneDay => "1D",
            Self::OneWeek => "1W",
            Self::OneMonth => "1M",
        }
    }
}

impl From<Interval> for tradingview::models::Interval {
    fn from(i: Interval) -> Self {
        match i {
            Interval::OneMinute => tradingview::models::Interval::OneMinute,
            Interval::ThreeMinutes => tradingview::models::Interval::ThreeMinutes,
            Interval::FiveMinutes => tradingview::models::Interval::FiveMinutes,
            Interval::FifteenMinutes => tradingview::models::Interval::FifteenMinutes,
            Interval::ThirtyMinutes => tradingview::models::Interval::ThirtyMinutes,
            Interval::FortyFiveMinutes => tradingview::models::Interval::FortyFiveMinutes,
            Interval::OneHour => tradingview::models::Interval::OneHour,
            Interval::TwoHours => tradingview::models::Interval::TwoHours,
            Interval::ThreeHours => tradingview::models::Interval::TwoHours,
            Interval::FourHours => tradingview::models::Interval::FourHours,
            Interval::OneDay => tradingview::models::Interval::OneDay,
            Interval::OneWeek => tradingview::models::Interval::OneWeek,
            Interval::OneMonth => tradingview::models::Interval::OneMonth,
        }
    }
}

impl From<tradingview::models::Interval> for Interval {
    fn from(i: tradingview::models::Interval) -> Self {
        match i {
            tradingview::models::Interval::OneMinute => Interval::OneMinute,
            tradingview::models::Interval::ThreeMinutes => Interval::ThreeMinutes,
            tradingview::models::Interval::FiveMinutes => Interval::FiveMinutes,
            tradingview::models::Interval::FifteenMinutes => Interval::FifteenMinutes,
            tradingview::models::Interval::ThirtyMinutes => Interval::ThirtyMinutes,
            tradingview::models::Interval::FortyFiveMinutes => Interval::FortyFiveMinutes,
            tradingview::models::Interval::OneHour => Interval::OneHour,
            tradingview::models::Interval::TwoHours => Interval::TwoHours,
            tradingview::models::Interval::FourHours => Interval::FourHours,
            tradingview::models::Interval::OneDay => Interval::OneDay,
            tradingview::models::Interval::OneWeek => Interval::OneWeek,
            tradingview::models::Interval::OneMonth => Interval::OneMonth,
            _ => Interval::OneDay,
        }
    }
}

/// Reporting cadence for fundamental financial metrics.
#[pyclass(eq, eq_int, from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FinancialPeriod {
    FiscalYear,
    FiscalQuarter,
    FiscalHalfYear,
    TrailingTwelveMonths,
    NoAggregation,
}

#[pymethods]
impl FinancialPeriod {
    #[getter]
    pub fn value(&self) -> &'static str {
        self.as_str()
    }

    #[getter]
    pub fn name(&self) -> &'static str {
        match self {
            Self::FiscalYear => "FiscalYear",
            Self::FiscalQuarter => "FiscalQuarter",
            Self::FiscalHalfYear => "FiscalHalfYear",
            Self::TrailingTwelveMonths => "TrailingTwelveMonths",
            Self::NoAggregation => "NoAggregation",
        }
    }

    fn __repr__(&self) -> String {
        format!("<FinancialPeriod.{}: '{}'>", self.name(), self.value())
    }

    fn __str__(&self) -> &'static str {
        self.as_str()
    }
}

impl FinancialPeriod {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::FiscalYear => "FY",
            Self::FiscalQuarter => "FQ",
            Self::FiscalHalfYear => "FH",
            Self::TrailingTwelveMonths => "TTM",
            Self::NoAggregation => "NOAGG",
        }
    }
}

impl From<FinancialPeriod> for tradingview::models::FinancialPeriod {
    fn from(p: FinancialPeriod) -> Self {
        match p {
            FinancialPeriod::FiscalYear => tradingview::models::FinancialPeriod::FiscalYear,
            FinancialPeriod::FiscalQuarter => tradingview::models::FinancialPeriod::FiscalQuarter,
            FinancialPeriod::FiscalHalfYear => tradingview::models::FinancialPeriod::FiscalHalfYear,
            FinancialPeriod::TrailingTwelveMonths => {
                tradingview::models::FinancialPeriod::TrailingTwelveMonths
            }
            FinancialPeriod::NoAggregation => {
                tradingview::models::FinancialPeriod::UnknownPeriod("NOAGG".to_string())
            }
        }
    }
}

impl From<tradingview::models::FinancialPeriod> for FinancialPeriod {
    fn from(p: tradingview::models::FinancialPeriod) -> Self {
        match p {
            tradingview::models::FinancialPeriod::FiscalYear => FinancialPeriod::FiscalYear,
            tradingview::models::FinancialPeriod::FiscalQuarter => FinancialPeriod::FiscalQuarter,
            tradingview::models::FinancialPeriod::FiscalHalfYear => FinancialPeriod::FiscalHalfYear,
            tradingview::models::FinancialPeriod::TrailingTwelveMonths => {
                FinancialPeriod::TrailingTwelveMonths
            }
            tradingview::models::FinancialPeriod::UnknownPeriod(s) => match s.as_str() {
                "FY" => FinancialPeriod::FiscalYear,
                "FQ" => FinancialPeriod::FiscalQuarter,
                "FH" => FinancialPeriod::FiscalHalfYear,
                "TTM" => FinancialPeriod::TrailingTwelveMonths,
                _ => FinancialPeriod::NoAggregation,
            },
        }
    }
}

/// Macroeconomic event importance level.
#[pyclass(eq, eq_int, from_py_object)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EconomicImportance {
    Low = -1,
    Medium = 0,
    High = 1,
}

#[pymethods]
impl EconomicImportance {
    #[getter]
    pub fn value(&self) -> i32 {
        *self as i32
    }

    #[getter]
    pub fn name(&self) -> &'static str {
        match self {
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
        }
    }

    fn __repr__(&self) -> String {
        format!("<EconomicImportance.{}: {}>", self.name(), self.value())
    }

    fn __str__(&self) -> &'static str {
        match self {
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
        }
    }
}

impl From<EconomicImportance> for tradingview::client::fin_calendar::EconomicImportance {
    fn from(imp: EconomicImportance) -> Self {
        match imp {
            EconomicImportance::Low => tradingview::client::fin_calendar::EconomicImportance::Low,
            EconomicImportance::Medium => {
                tradingview::client::fin_calendar::EconomicImportance::Medium
            }
            EconomicImportance::High => tradingview::client::fin_calendar::EconomicImportance::High,
        }
    }
}

impl From<tradingview::client::fin_calendar::EconomicImportance> for EconomicImportance {
    fn from(imp: tradingview::client::fin_calendar::EconomicImportance) -> Self {
        match imp {
            tradingview::client::fin_calendar::EconomicImportance::Low => EconomicImportance::Low,
            tradingview::client::fin_calendar::EconomicImportance::Medium => {
                EconomicImportance::Medium
            }
            tradingview::client::fin_calendar::EconomicImportance::High => EconomicImportance::High,
        }
    }
}
