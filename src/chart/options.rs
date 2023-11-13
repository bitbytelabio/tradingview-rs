use iso_currency::Currency;

use crate::{
    chart::ChartOptions,
    models::{ Interval, MarketAdjustment, SessionType, pine_indicator::ScriptType },
};

use super::StudyOptions;

impl ChartOptions {
    pub fn new(symbol: &str, interval: Interval) -> Self {
        Self {
            symbol: symbol.to_string(),
            interval,
            bar_count: 5000,
            ..Default::default()
        }
    }

    pub fn bar_count(mut self, bar_count: u64) -> Self {
        self.bar_count = bar_count;
        self
    }

    pub fn replay_mode(mut self, replay_mode: bool) -> Self {
        self.replay_mode = replay_mode;
        self
    }

    pub fn replay_from(mut self, replay_from: i64) -> Self {
        self.replay_from = replay_from;
        self
    }

    pub fn replay_session_id(mut self, replay_session_id: &str) -> Self {
        self.replay_session = Some(replay_session_id.to_string());
        self
    }

    pub fn range(mut self, range: &str) -> Self {
        self.range = Some(range.to_string());
        self
    }

    pub fn from(mut self, from: u64) -> Self {
        self.from = Some(from);
        self
    }

    pub fn to(mut self, to: u64) -> Self {
        self.to = Some(to);
        self
    }

    pub fn adjustment(mut self, adjustment: MarketAdjustment) -> Self {
        self.adjustment = Some(adjustment);
        self
    }

    pub fn currency(mut self, currency: Currency) -> Self {
        self.currency = Some(currency);
        self
    }

    pub fn session_type(mut self, session_type: SessionType) -> Self {
        self.session_type = Some(session_type);
        self
    }

    pub fn study_config(
        mut self,
        script_id: &str,
        script_version: &str,
        script_type: ScriptType
    ) -> Self {
        self.study_config = Some(StudyOptions {
            script_id: script_id.to_string(),
            script_version: script_version.to_string(),
            script_type,
        });
        self
    }
}
