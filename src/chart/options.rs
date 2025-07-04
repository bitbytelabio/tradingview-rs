use crate::models::{Interval, MarketAdjustment, SessionType, pine_indicator::ScriptType};
use bon::Builder;
use iso_currency::Currency;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, Builder)]
pub struct ChartOptions {
    #[builder(default)]
    pub symbol: String,
    #[builder(default)]
    pub exchange: String,
    #[builder(default = Interval::OneDay)]
    pub interval: Interval,
    #[builder(default = 500_000)]
    pub bar_count: u64,
    pub range: Option<String>,
    pub from: Option<u64>,
    pub to: Option<u64>,
    #[builder(default = false)]
    pub replay_mode: bool,
    #[builder(default = 0)]
    pub replay_from: i64,
    pub replay_session: Option<String>,
    pub adjustment: Option<MarketAdjustment>,
    pub currency: Option<Currency>,
    pub session_type: Option<SessionType>,
    pub study_config: Option<StudyOptions>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub enum Range {
    FromTo(u64, u64),
    OneDay,
    FiveDays,
    OneMonth,
    ThreeMonths,
    SixMonths,
    YearToDate,
    OneYear,
    FiveYears,
    All,
}

impl From<Range> for String {
    fn from(range: Range) -> Self {
        match range {
            Range::FromTo(from, to) => format!("r,{from}:{to}"),
            Range::OneDay => "1D".to_string(),
            Range::FiveDays => "5d".to_string(),
            Range::OneMonth => "1M".to_string(),
            Range::ThreeMonths => "3M".to_string(),
            Range::SixMonths => "6M".to_string(),
            Range::YearToDate => "YTD".to_string(),
            Range::OneYear => "12M".to_string(),
            Range::FiveYears => "60M".to_string(),
            Range::All => "ALL".to_string(),
        }
    }
}

#[derive(Default, Debug, Clone, Deserialize, Serialize, Builder)]
pub struct StudyOptions {
    pub script_id: String,
    pub script_version: String,
    pub script_type: ScriptType,
}

impl Default for ChartOptions {
    fn default() -> Self {
        Self::builder().build()
    }
}

impl ChartOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn new_with(symbol: &str, exchange: &str, interval: Interval) -> Self {
        Self {
            symbol: symbol.to_string(),
            exchange: exchange.to_string(),
            interval,
            bar_count: 500_000,
            ..Default::default()
        }
    }

    pub fn symbol(mut self, symbol: &str) -> Self {
        self.symbol = symbol.to_string();
        self
    }

    pub fn exchange(mut self, exchange: &str) -> Self {
        self.exchange = exchange.to_string();
        self
    }

    pub fn interval(mut self, interval: Interval) -> Self {
        self.interval = interval;
        self
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

    /// range: |r,1626220800:1628640000|1D|5d|1M|3M|6M|YTD|12M|60M|ALL|
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
        script_type: ScriptType,
    ) -> Self {
        self.study_config = Some(StudyOptions {
            script_id: script_id.to_string(),
            script_version: script_version.to_string(),
            script_type,
        });
        self
    }
}
