use crate::models::{Interval, MarketAdjustment, SessionType, pine_indicator::ScriptType};
use bon::Builder;
use iso_currency::Currency;
use serde::{Deserialize, Serialize};
use ustr::Ustr;

#[derive(Debug, Clone, Deserialize, Serialize, Builder, Copy)]
pub struct ChartOptions {
    #[builder(default)]
    pub symbol: Ustr,
    #[builder(default)]
    pub exchange: Ustr,
    #[builder(default = Interval::OneDay)]
    pub interval: Interval,
    #[builder(default = 500_000)]
    pub bar_count: u64,
    pub range: Option<Ustr>,
    pub from: Option<u64>,
    pub to: Option<u64>,
    #[builder(default = false)]
    pub replay_mode: bool,
    #[builder(default = 0)]
    pub replay_from: i64,
    pub replay_session: Option<Ustr>,
    pub adjustment: Option<MarketAdjustment>,
    pub currency: Option<Currency>,
    pub session_type: Option<SessionType>,
    pub study_config: Option<StudyOptions>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, Copy)]
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

impl Into<Ustr> for Range {
    fn into(self) -> Ustr {
        match self {
            Range::FromTo(from, to) => Ustr::from(&format!("r,{from}:{to}")),
            Range::OneDay => Ustr::from("1D"),
            Range::FiveDays => Ustr::from("5d"),
            Range::OneMonth => Ustr::from("1M"),
            Range::ThreeMonths => Ustr::from("3M"),
            Range::SixMonths => Ustr::from("6M"),
            Range::YearToDate => Ustr::from("YTD"),
            Range::OneYear => Ustr::from("12M"),
            Range::FiveYears => Ustr::from("60M"),
            Range::All => Ustr::from("ALL"),
        }
    }
}

#[derive(Default, Debug, Clone, Deserialize, Serialize, Builder, Copy)]
pub struct StudyOptions {
    pub script_id: Ustr,
    pub script_version: Ustr,
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
            symbol: Ustr::from(symbol),
            exchange: Ustr::from(exchange),
            interval,
            bar_count: 500_000,
            ..Default::default()
        }
    }

    pub fn symbol(mut self, symbol: &str) -> Self {
        self.symbol = Ustr::from(symbol);
        self
    }

    pub fn exchange(mut self, exchange: &str) -> Self {
        self.exchange = Ustr::from(exchange);
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
        self.replay_session = Some(Ustr::from(replay_session_id));
        self
    }

    /// range: |r,1626220800:1628640000|1D|5d|1M|3M|6M|YTD|12M|60M|ALL|
    pub fn range(mut self, range: &str) -> Self {
        self.range = Some(Ustr::from(range));
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
            script_id: Ustr::from(script_id),
            script_version: Ustr::from(script_version),
            script_type,
        });
        self
    }
}
