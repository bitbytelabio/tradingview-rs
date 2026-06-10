//! Chart session configuration and Pine Script study support.
//!
//! Chart sessions are the primary abstraction for requesting real-time data
//! from TradingView. Each chart session specifies a symbol, interval, and
//! optional study configuration.
//!
//! # Key Types
//!
//! - [`ChartOptions`] — Full configuration for a chart data subscription
//!   (symbol, exchange, interval, bar count, replay mode, studies).
//! - [`StudyOptions`] — Pine Script indicator configuration (script ID,
//!   version, type).
//! - [`study`] — Built-in Pine Script indicator catalogs and interaction.
//!
//! [`ChartOptions`]: crate::chart::ChartOptions
//! [`StudyOptions`]: crate::chart::StudyOptions

pub(crate) mod options;
pub mod study;
pub(crate) mod utils;

mod models;

pub use models::*;
pub use options::ChartOptions;
pub use options::StudyOptions;
pub use utils::*;

pub const CHART_SESSION_IDX: usize = 0;

pub const STUDY_IDX: usize = 1;
pub const SERIES_IDX: usize = 1;

pub const SERIES_DATA_IDX: usize = 2;
pub const SERIES_STUDY_IDX: usize = 2;
