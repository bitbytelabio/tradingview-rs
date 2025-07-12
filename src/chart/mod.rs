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
