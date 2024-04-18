use iso_currency::Currency;

use crate::models::{pine_indicator::ScriptType, Interval, MarketAdjustment, SessionType};

pub mod models;
pub(crate) mod options;
pub mod study;
pub(crate) mod utils;

#[derive(Default, Debug, Clone)]
pub struct ChartOptions {
    // Required
    pub symbol: String,
    pub interval: Interval,
    pub(crate) bar_count: u64,

    pub(crate) range: Option<String>,
    pub(crate) from: Option<u64>,
    pub(crate) to: Option<u64>,
    pub(crate) replay_mode: bool,
    pub(crate) replay_from: i64,
    pub(crate) replay_session: Option<String>,
    pub(crate) adjustment: Option<MarketAdjustment>,
    pub(crate) currency: Option<Currency>,
    pub(crate) session_type: Option<SessionType>,
    pub study_config: Option<StudyOptions>,
}

#[derive(Default, Debug, Clone)]
pub struct StudyOptions {
    pub script_id: String,
    pub script_version: String,
    pub script_type: ScriptType,
}
