use crate::{MarketSymbol, MarketType, websocket::SeriesInfo};
use bon::Builder;
use chrono::{DateTime, Utc};
use iso_currency::Currency;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ustr::Ustr;

#[derive(Debug, Clone, Deserialize, Serialize, Copy, PartialEq, Eq, Hash)]
pub enum ChartType {
    HeikinAshi,
    Renko,
    LineBreak,
    Kagi,
    PointAndFigure,
    Range,
}

impl std::fmt::Display for ChartType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let chart_type = match self {
            ChartType::HeikinAshi => "BarSetHeikenAshi@tv-basicstudies-60!",
            ChartType::Renko => "BarSetRenko@tv-prostudies-40!",
            ChartType::LineBreak => "BarSetPriceBreak@tv-prostudies-34!",
            ChartType::Kagi => "BarSetKagi@tv-prostudies-34!",
            ChartType::PointAndFigure => "BarSetPnF@tv-prostudies-34!",
            ChartType::Range => "BarSetRange@tv-basicstudies-72!",
        };
        write!(f, "{chart_type}")
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, Builder)]
pub struct ChartHistoricalData {
    pub symbol_info: SymbolInfo,
    pub series_info: SeriesInfo,
    pub data: Vec<DataPoint>,
}

impl ChartHistoricalData {
    pub fn new() -> Self {
        Self {
            symbol_info: SymbolInfo::default(),
            series_info: SeriesInfo::default(),
            data: Vec::new(),
        }
    }
}

impl Default for ChartHistoricalData {
    fn default() -> Self {
        Self::new()
    }
}

impl PriceIterable for ChartHistoricalData {
    type Item = DataPoint;

    fn to_vec(&self) -> impl Iterator<Item = &Self::Item> + '_ {
        self.data.to_vec()
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChartResponseData {
    #[serde(default)]
    pub node: Option<Ustr>,
    #[serde(rename(deserialize = "s"))]
    pub series: Vec<DataPoint>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StudyResponseData {
    #[serde(default)]
    pub node: Option<Ustr>,
    #[serde(rename(deserialize = "st"))]
    pub studies: Vec<DataPoint>,
    #[serde(rename(deserialize = "ns"))]
    pub raw_graphics: GraphicDataResponse,
}

// TODO: Implement graphic parser for indexes response
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GraphicDataResponse {
    pub d: Ustr,
    pub indexes: Value,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Debug, Default)]
pub struct DataPoint {
    #[serde(rename(deserialize = "i"))]
    pub index: i64,
    #[serde(rename(deserialize = "v"))]
    pub value: Vec<f64>,
}

pub trait PriceIterable {
    type Item: OHLCV;

    fn to_vec(&self) -> impl Iterator<Item = &Self::Item> + '_;

    fn closes(&self) -> impl Iterator<Item = f64> + '_ {
        self.to_vec().map(|dp| dp.close())
    }

    fn opens(&self) -> impl Iterator<Item = f64> + '_ {
        self.to_vec().map(|dp| dp.open())
    }

    fn highs(&self) -> impl Iterator<Item = f64> + '_ {
        self.to_vec().map(|dp| dp.high())
    }

    fn lows(&self) -> impl Iterator<Item = f64> + '_ {
        self.to_vec().map(|dp| dp.low())
    }

    fn volumes(&self) -> impl Iterator<Item = f64> + '_ {
        self.to_vec().map(|dp| dp.volume())
    }

    fn datetimes(&self) -> impl Iterator<Item = DateTime<Utc>> + '_ {
        self.to_vec().map(|dp| dp.datetime())
    }

    fn timestamps(&self) -> impl Iterator<Item = i64> + '_ {
        self.to_vec().map(|dp| dp.timestamp())
    }
}

impl PriceIterable for Vec<DataPoint> {
    type Item = DataPoint;

    fn to_vec(&self) -> impl Iterator<Item = &Self::Item> + '_ {
        self.iter()
    }
}

pub trait OHLCV {
    fn datetime(&self) -> DateTime<Utc>;
    fn timestamp(&self) -> i64;
    fn open(&self) -> f64;
    fn high(&self) -> f64;
    fn low(&self) -> f64;
    fn close(&self) -> f64;
    fn volume(&self) -> f64;
    fn validate(&self) -> bool {
        !(self.close() > self.high() || self.close() < self.low() || self.high() < self.low())
            && self.close() > 0.
            && self.open() > 0.
            && self.high() > 0.
            && self.low() > 0.
            && self.close().is_finite()
            && self.open().is_finite()
            && self.high().is_finite()
            && self.low().is_finite()
            && (self.volume().is_nan() || self.volume() >= 0.0)
    }

    fn tr(&self, prev_candle: &dyn OHLCV) -> f64 {
        self.tr_close(prev_candle.close())
    }

    fn tr_close(&self, prev_close: f64) -> f64 {
        self.high().max(prev_close) - self.low().min(prev_close)
    }

    fn clv(&self) -> f64 {
        if self.high() == self.low() {
            0.
        } else {
            let twice: f64 = 2.;
            (twice.mul_add(self.close(), -self.low()) - self.high()) / (self.high() - self.low())
        }
    }

    fn ohlc4(&self) -> f64 {
        (self.high() + self.low() + self.close() + self.open()) * 0.25
    }

    fn hl2(&self) -> f64 {
        (self.high() + self.low()) * 0.5
    }

    fn tp(&self) -> f64 {
        (self.high() + self.low() + self.close()) / 3.
    }

    fn volumed_price(&self) -> f64 {
        self.tp() * self.volume()
    }

    fn is_rising(&self) -> bool {
        self.close() > self.open()
    }

    fn is_falling(&self) -> bool {
        self.close() < self.open()
    }
}

impl OHLCV for DataPoint {
    fn datetime(&self) -> DateTime<Utc> {
        let timestamp = self.value[0] as i64;
        DateTime::<Utc>::from_timestamp(timestamp, 0).expect("Invalid timestamp")
    }

    fn timestamp(&self) -> i64 {
        self.value[0] as i64
    }

    fn open(&self) -> f64 {
        if !(self.value.len() >= 5) {
            return f64::NAN;
        }
        self.value[1]
    }

    fn high(&self) -> f64 {
        if !(self.value.len() >= 5) {
            return f64::NAN;
        }
        self.value[2]
    }

    fn low(&self) -> f64 {
        if !(self.value.len() >= 5) {
            return f64::NAN;
        }
        self.value[3]
    }

    fn close(&self) -> f64 {
        if !(self.value.len() >= 5) {
            return f64::NAN;
        }
        self.value[4]
    }

    fn volume(&self) -> f64 {
        if !(self.value.len() >= 5) {
            return f64::NAN;
        }
        self.value[5]
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChartDataChanges {
    pub changes: Vec<f64>,
    pub index: i64,
    pub index_diff: Vec<Value>,
    pub marks: Vec<Value>,
    pub zoffset: i64,
}

#[derive(Clone, PartialEq, Serialize, Hash, Debug, Default, Copy)]
pub struct SeriesCompletedMessage {
    #[serde(default)]
    pub id: Ustr,
    #[serde(default)]
    pub update_mode: Ustr,
}

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug, Default, Builder, Copy)]
pub struct MarketTicker {
    pub symbol: Ustr,
    pub exchange: Ustr,
    pub currency: Option<Currency>,
    pub country: Option<Currency>,
    pub market_type: Option<MarketType>,
}

impl MarketTicker {
    pub fn new(symbol: &str, exchange: &str) -> Self {
        Self {
            symbol: Ustr::from(symbol),
            exchange: Ustr::from(exchange),
            currency: None,
            country: None,
            market_type: None,
        }
    }
}

impl MarketSymbol for MarketTicker {
    fn symbol(&self) -> &str {
        &self.symbol
    }

    fn exchange(&self) -> &str {
        &self.exchange
    }

    fn id(&self) -> String {
        format!("{}:{}", self.exchange, self.symbol)
    }

    fn new<S: Into<String>>(symbol: S, exchange: S) -> Self {
        Self {
            symbol: Ustr::from(&symbol.into()),
            exchange: Ustr::from(&exchange.into()),
            currency: None,
            country: None,
            market_type: None,
        }
    }
}

impl From<&SymbolInfo> for MarketTicker {
    fn from(symbol_info: &SymbolInfo) -> Self {
        Self {
            symbol: symbol_info.name.clone(),
            exchange: symbol_info.exchange.clone(),
            currency: Currency::from_code(symbol_info.currency_id.as_str()),
            country: None,
            market_type: Some(MarketType::from(symbol_info.market_type.as_str())),
        }
    }
}

impl From<SymbolInfo> for MarketTicker {
    fn from(val: SymbolInfo) -> Self {
        MarketTicker::from(&val)
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug, Default)]
#[serde(default)]
pub struct SymbolInfo {
    #[serde(rename(deserialize = "pro_name"))]
    pub id: Ustr,

    #[serde(rename(deserialize = "original_name"))]
    pub original_name: Ustr,

    pub name: Ustr,
    pub exchange: Ustr,
    pub description: Ustr,

    #[serde(rename = "business_description")]
    pub business_description: Ustr,

    #[serde(rename = "listed_exchange")]
    pub listed_exchange: Ustr,

    #[serde(rename = "provider_id")]
    pub provider_id: Ustr,

    #[serde(rename = "base_currency")]
    pub base_currency: Ustr,

    #[serde(rename = "base_currency_id")]
    pub base_currency_id: Ustr,

    #[serde(rename = "total_revenue")]
    pub total_revenue: f64,

    #[serde(rename = "price_earnings_ttm")]
    pub price_earnings_ttm: f64,

    #[serde(rename = "currency_id")]
    pub currency_id: Ustr,

    #[serde(rename = "currency_code")]
    pub currency_code: Ustr,

    pub session_holidays: Ustr,

    pub subsessions: Vec<Subsession>,

    pub timezone: Ustr,

    #[serde(rename(deserialize = "type"))]
    pub market_type: Ustr,

    pub typespecs: Vec<Ustr>,

    pub aliases: Vec<Ustr>,

    pub total_shares_outstanding_calculated: f64,

    pub market_cap_basic: f64,

    pub earnings_release_date: i64,

    pub base_name: Vec<Ustr>,

    pub sector: Ustr,

    pub current_session: Ustr,

    pub founded: u16,

    pub last_annual_eps: f64,

    pub fractional: bool,

    pub industry: Ustr,
}

impl MarketSymbol for SymbolInfo {
    fn symbol(&self) -> &str {
        self.name.as_str()
    }

    fn exchange(&self) -> &str {
        self.exchange.as_str()
    }

    fn id(&self) -> String {
        self.id.to_string()
    }

    fn new<S: Into<String>>(symbol: S, exchange: S) -> Self {
        Self {
            name: Ustr::from(&symbol.into()),
            exchange: Ustr::from(&exchange.into()),
            ..Default::default()
        }
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize, Hash, Debug, Default, Copy)]
#[serde(rename_all = "camelCase", default)]
pub struct Subsession {
    pub id: Ustr,
    pub description: Ustr,
    pub private: bool,
    pub session: Ustr,
    #[serde(rename(deserialize = "session-display"))]
    pub session_display: Ustr,
}
