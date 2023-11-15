use iso_currency::Currency;

use crate::models::{ MarketAdjustment, SessionType, pine_indicator::ScriptType, Interval };

pub mod session;
pub mod study;
pub mod models;
pub(crate) mod options;
pub(crate) mod utils;

#[derive(Default, Debug, Clone)]
pub struct ChartOptions {
    // Required
    pub symbol: String,
    pub interval: Interval,
    bar_count: u64,

    range: Option<String>,
    from: Option<u64>,
    to: Option<u64>,
    replay_mode: bool,
    replay_from: i64,
    replay_session: Option<String>,
    adjustment: Option<MarketAdjustment>,
    currency: Option<Currency>,
    session_type: Option<SessionType>,
    study_config: Option<StudyOptions>,
}

#[derive(Default, Debug, Clone)]
pub struct StudyOptions {
    pub script_id: String,
    pub script_version: String,
    pub script_type: ScriptType,
}
