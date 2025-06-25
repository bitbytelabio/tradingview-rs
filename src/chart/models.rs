use crate::{MarketSymbol, MarketType, websocket::SeriesInfo};
use bon::Builder;
use chrono::{DateTime, Utc};
use iso_currency::Currency;
use serde::{Deserialize, Serialize};
use serde_json::Value;

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

#[derive(Debug, Clone, Deserialize)]
pub struct ChartResponseData {
    #[serde(default)]
    pub node: Option<String>,
    #[serde(rename(deserialize = "s"))]
    pub series: Vec<DataPoint>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StudyResponseData {
    #[serde(default)]
    pub node: Option<String>,
    #[serde(rename(deserialize = "st"))]
    pub studies: Vec<DataPoint>,
    #[serde(rename(deserialize = "ns"))]
    pub raw_graphics: GraphicDataResponse,
}

// TODO: Implement graphic parser for indexes response
#[derive(Debug, Clone, Deserialize)]
pub struct GraphicDataResponse {
    pub d: String,
    pub indexes: Value,
}

#[cfg_attr(not(feature = "protobuf"), derive(Debug, Default))]
#[cfg_attr(feature = "protobuf", derive(prost::Message))]
#[derive(Clone, Deserialize, Serialize, PartialEq)]
pub struct DataPoint {
    #[cfg_attr(feature = "protobuf", prost(int64, tag = "1"))]
    #[serde(rename(deserialize = "i"))]
    pub index: i64,
    #[cfg_attr(feature = "protobuf", prost(double, repeated, tag = "2"))]
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
    fn is_ohlcv(&self) -> bool;
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
        if !(self.is_ohlcv()) {
            return f64::NAN;
        }
        self.value[1]
    }

    fn high(&self) -> f64 {
        if !(self.is_ohlcv()) {
            return f64::NAN;
        }
        self.value[2]
    }

    fn low(&self) -> f64 {
        if !(self.is_ohlcv()) {
            return f64::NAN;
        }
        self.value[3]
    }

    fn close(&self) -> f64 {
        if !(self.is_ohlcv()) {
            return f64::NAN;
        }
        self.value[4]
    }

    fn volume(&self) -> f64 {
        if !self.is_ohlcv() {
            return f64::NAN;
        }
        self.value[5]
    }

    fn is_ohlcv(&self) -> bool {
        self.value.len() == 6
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

#[cfg_attr(not(feature = "protobuf"), derive(Debug, Default))]
#[cfg_attr(feature = "protobuf", derive(prost::Message))]
#[derive(Clone, PartialEq, Serialize, Hash)]
pub struct SeriesCompletedMessage {
    #[cfg_attr(feature = "protobuf", prost(string, tag = "1"))]
    #[serde(default)]
    pub id: String,
    #[cfg_attr(feature = "protobuf", prost(string, tag = "2"))]
    #[serde(default)]
    pub update_mode: String,
}

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug, Default, Builder)]
pub struct MarketTicker {
    pub symbol: String,
    pub exchange: String,
    pub currency: Option<Currency>,
    pub country: Option<Currency>,
    pub market_type: Option<MarketType>,
}

impl MarketTicker {
    pub fn new(symbol: &str, exchange: &str) -> Self {
        Self {
            symbol: symbol.to_string(),
            exchange: exchange.to_string(),
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

    fn currency(&self) -> &str {
        self.currency.as_ref().map_or("USD", |c| c.code())
    }

    fn id(&self) -> String {
        format!("{}:{}", self.exchange, self.symbol)
    }

    fn market_type(&self) -> MarketType {
        self.market_type.unwrap_or(MarketType::All)
    }

    fn new<S: Into<String>>(symbol: S, exchange: S) -> Self {
        Self {
            symbol: symbol.into(),
            exchange: exchange.into(),
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
    pub id: String,

    #[serde(rename(deserialize = "original_name"))]
    pub original_name: String,

    pub name: String,
    pub exchange: String,
    pub description: String,

    #[serde(rename = "business_description")]
    pub business_description: String,

    #[serde(rename = "listed_exchange")]
    pub listed_exchange: String,

    #[serde(rename = "provider_id")]
    pub provider_id: String,

    #[serde(rename = "base_currency")]
    pub base_currency: String,

    #[serde(rename = "base_currency_id")]
    pub base_currency_id: String,

    #[serde(rename = "total_revenue")]
    pub total_revenue: f64,

    #[serde(rename = "price_earnings_ttm")]
    pub price_earnings_ttm: f64,

    #[serde(rename = "currency_id")]
    pub currency_id: String,

    #[serde(rename = "currency_code")]
    pub currency_code: String,

    pub session_holidays: String,

    pub subsessions: Vec<Subsession>,

    pub timezone: String,

    #[serde(rename(deserialize = "type"))]
    pub market_type: String,

    pub typespecs: Vec<String>,

    pub aliases: Vec<String>,

    pub total_shares_outstanding_calculated: f64,

    pub market_cap_basic: f64,

    pub earnings_release_date: i64,

    pub base_name: Vec<String>,

    pub sector: String,

    pub current_session: String,

    pub founded: u16,

    pub last_annual_eps: f64,

    pub fractional: bool,

    pub industry: String,
}

impl MarketSymbol for SymbolInfo {
    fn symbol(&self) -> &str {
        self.name.as_str()
    }

    fn exchange(&self) -> &str {
        self.exchange.as_str()
    }

    fn currency(&self) -> &str {
        &self.currency_id
    }

    fn id(&self) -> String {
        self.id.clone()
    }

    fn market_type(&self) -> MarketType {
        MarketType::from(self.market_type.as_str())
    }

    fn new<S: Into<String>>(symbol: S, exchange: S) -> Self {
        Self {
            name: symbol.into(),
            exchange: exchange.into(),
            ..Default::default()
        }
    }
}

#[cfg_attr(not(feature = "protobuf"), derive(Debug, Default))]
#[cfg_attr(feature = "protobuf", derive(prost::Message))]
#[derive(Clone, PartialEq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "camelCase", default)]
pub struct Subsession {
    #[cfg_attr(feature = "protobuf", prost(string, tag = "1"))]
    pub id: String,
    #[cfg_attr(feature = "protobuf", prost(string, tag = "2"))]
    pub description: String,
    #[cfg_attr(feature = "protobuf", prost(bool, tag = "3"))]
    pub private: bool,
    #[cfg_attr(feature = "protobuf", prost(string, tag = "4"))]
    pub session: String,
    #[cfg_attr(feature = "protobuf", prost(string, tag = "5"))]
    #[serde(rename(deserialize = "session-display"))]
    pub session_display: String,
}
