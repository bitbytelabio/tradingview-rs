use crate::{
    models::{pine_indicator::ScriptType, Interval, MarketAdjustment, SessionType},
    Error,
};
use bon::Builder;
use iso_currency::Currency;
use serde::{Deserialize, Serialize};
use std::fmt;
use ustr::Ustr;

/// Configuration for a real-time chart data subscription.
///
/// Specifies the instrument, time interval, bar count, replay settings,
/// and optional study configuration for a TradingView WebSocket chart session.
///
/// # Construction
///
/// Use the builder pattern via [`bon`]:
///
/// ```rust
/// use tradingview::{ChartOptions, Interval};
///
/// let opts = ChartOptions::builder()
///     .symbol("BTCUSDT")
///     .exchange("BINANCE")
///     .interval(Interval::OneHour)
///     .bar_count(500)
///     .build()
///     .unwrap();
/// ```
///
/// Alternatively, pass an instrument string in `"EXCHANGE:SYMBOL"` format:
///
/// ```rust
/// let opts = ChartOptions::builder()
///     .instrument("BINANCE:BTCUSDT")
///     .build()
///     .unwrap();
/// ```
#[derive(Debug, Clone, Deserialize, Serialize, Copy)]
pub struct ChartOptions {
    pub symbol: Option<Ustr>,
    pub exchange: Option<Ustr>,
    pub interval: Interval,
    pub bar_count: u64,
    pub range: Option<Range>,
    pub replay_mode: bool,
    pub replay_from: i64,
    pub replay_session: Option<Ustr>,
    pub adjustment: Option<MarketAdjustment>,
    pub currency: Option<Currency>,
    pub session_type: Option<SessionType>,
    pub study_config: Option<StudyOptions>,
}

/// Data range specifier for chart subscriptions.
///
/// Determines how much historical data to request. `FromTo(u64, u64)` specifies
/// a custom Unix-timestamp range; the named variants are convenience presets.
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

impl Range {
    pub fn from_to(from: u64, to: u64) -> Self {
        Range::FromTo(from, to)
    }

    pub fn one_day() -> Self {
        Range::OneDay
    }

    pub fn five_days() -> Self {
        Range::FiveDays
    }

    pub fn one_month() -> Self {
        Range::OneMonth
    }

    pub fn three_months() -> Self {
        Range::ThreeMonths
    }

    pub fn six_months() -> Self {
        Range::SixMonths
    }

    pub fn year_to_date() -> Self {
        Range::YearToDate
    }

    pub fn one_year() -> Self {
        Range::OneYear
    }

    pub fn five_years() -> Self {
        Range::FiveYears
    }

    pub fn all() -> Self {
        Range::All
    }
}

impl fmt::Display for Range {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Range::FromTo(from, to) => write!(f, "r,{from}:{to}"),
            Range::OneDay => write!(f, "1D"),
            Range::FiveDays => write!(f, "5d"),
            Range::OneMonth => write!(f, "1M"),
            Range::ThreeMonths => write!(f, "3M"),
            Range::SixMonths => write!(f, "6M"),
            Range::YearToDate => write!(f, "YTD"),
            Range::OneYear => write!(f, "12M"),
            Range::FiveYears => write!(f, "60M"),
            Range::All => write!(f, "ALL"),
        }
    }
}

impl From<Range> for Ustr {
    fn from(val: Range) -> Self {
        match val {
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

/// Configuration for a Pine Script study (indicator) within a chart session.
///
/// A study is identified by its `script_id` and `script_version`. The
/// `script_type` determines how the study data is delivered (per-candle or
/// as a standalone series).
#[derive(Default, Debug, Clone, Deserialize, Serialize, Builder, Copy)]
pub struct StudyOptions {
    pub script_id: Ustr,
    pub script_version: Ustr,
    pub script_type: ScriptType,
}

impl Default for ChartOptions {
    fn default() -> Self {
        Self::builder()
            .build()
            .expect("Failed to create default ChartOptions")
    }
}

#[bon::bon]
impl ChartOptions {
    #[builder]
    pub fn new(
        instrument: Option<&str>,
        symbol: Option<&str>,
        exchange: Option<&str>,
        #[builder(default = Interval::OneDay)] interval: Interval,
        #[builder(default = 500_000)] bar_count: u64,
        range: Option<Range>,
        #[builder(default = false)] replay_mode: bool,
        #[builder(default = 0)] replay_from: i64,
        replay_session: Option<&str>,
        adjustment: Option<MarketAdjustment>,
        currency: Option<Currency>,
        session_type: Option<SessionType>,
        study_config: Option<StudyOptions>,
    ) -> Result<Self, Error> {
        let (validated_exchange, validated_symbol) =
            Self::validate_instrument(instrument, symbol, exchange)?;

        Ok(Self {
            symbol: validated_symbol,
            exchange: validated_exchange,
            interval,
            bar_count,
            range,
            replay_mode,
            replay_from,
            replay_session: replay_session.map(Ustr::from),
            adjustment,
            currency,
            session_type,
            study_config,
        })
    }

    fn validate_instrument(
        instrument: Option<&str>,
        symbol: Option<&str>,
        exchange: Option<&str>,
    ) -> Result<(Option<Ustr>, Option<Ustr>), String> {
        match (instrument, symbol, exchange) {
            // Case 1: Only instrument provided
            (Some(instrument), None, None) => {
                if instrument.trim().is_empty() {
                    return Err("Instrument cannot be empty or whitespace only".to_string());
                }

                let parts: Vec<&str> = instrument.split(':').collect();
                if parts.len() != 2 {
                    return Err("Instrument must be in format 'EXCHANGE:SYMBOL'".to_string());
                }

                let exchange_part = parts[0].trim();
                let symbol_part = parts[1].trim();

                if exchange_part.is_empty() || symbol_part.is_empty() {
                    return Err(
                        "Both exchange and symbol parts must be non-empty in instrument"
                            .to_string(),
                    );
                }

                // Validate characters (alphanumeric + common trading symbols)
                if !Self::is_valid_identifier(exchange_part)
                    || !Self::is_valid_identifier(symbol_part)
                {
                    return Err("Exchange and symbol must contain only alphanumeric characters, hyphens, dots, and underscores".to_string());
                }

                Ok((
                    Some(Ustr::from(exchange_part)),
                    Some(Ustr::from(symbol_part)),
                ))
            }

            // Case 2: Both symbol and exchange provided
            (None, Some(symbol), Some(exchange)) => {
                let symbol = symbol.trim();
                let exchange = exchange.trim();

                if symbol.is_empty() {
                    return Err("Symbol cannot be empty or whitespace only".to_string());
                }
                if exchange.is_empty() {
                    return Err("Exchange cannot be empty or whitespace only".to_string());
                }

                if !Self::is_valid_identifier(symbol) || !Self::is_valid_identifier(exchange) {
                    return Err("Symbol and exchange must contain only alphanumeric characters, hyphens, dots, and underscores".to_string());
                }

                Ok((Some(Ustr::from(exchange)), Some(Ustr::from(symbol))))
            }

            // Case 3: Invalid combinations
            (None, None, None) => {
                Err("Either instrument OR both symbol and exchange must be provided".to_string())
            }
            (Some(_), Some(_), _) | (Some(_), _, Some(_)) => {
                Err("Cannot provide instrument together with symbol or exchange".to_string())
            }
            (None, Some(_), None) | (None, None, Some(_)) => {
                Err("Symbol and exchange must be provided together".to_string())
            }
        }
    }

    fn is_valid_identifier(s: &str) -> bool {
        !s.is_empty()
            && s.chars().all(|c| {
                c.is_alphanumeric()
                    || c == '$'
                    || c == '%'
                    || c == '#'
                    || c == '*'
                    || c == '('
                    || c == ')'
                    || c == ':'
            })
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
    pub fn range(mut self, range: Range) -> Self {
        self.range = Some(range);
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
