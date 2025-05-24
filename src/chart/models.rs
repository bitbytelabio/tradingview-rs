use crate::{Error, MarketSymbol, Result, websocket::SeriesInfo};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Deserialize, Serialize)]
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

#[derive(Debug, Clone, Deserialize, Serialize)]
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

pub trait DataPointIter {
    fn closes(&self) -> impl Iterator<Item = f64> + '_;
    fn opens(&self) -> impl Iterator<Item = f64> + '_;
    fn highs(&self) -> impl Iterator<Item = f64> + '_;
    fn lows(&self) -> impl Iterator<Item = f64> + '_;
    fn volumes(&self) -> impl Iterator<Item = f64> + '_;
    fn datetimes(&self) -> impl Iterator<Item = DateTime<Utc>> + '_;
    fn timestamps(&self) -> impl Iterator<Item = i64> + '_;
}

impl DataPointIter for Vec<DataPoint> {
    fn closes(&self) -> impl Iterator<Item = f64> + '_ {
        self.iter().map(|dp| dp.close())
    }

    fn opens(&self) -> impl Iterator<Item = f64> + '_ {
        self.iter().map(|dp| dp.open())
    }

    fn highs(&self) -> impl Iterator<Item = f64> + '_ {
        self.iter().map(|dp| dp.high())
    }

    fn lows(&self) -> impl Iterator<Item = f64> + '_ {
        self.iter().map(|dp| dp.low())
    }

    fn volumes(&self) -> impl Iterator<Item = f64> + '_ {
        self.iter().map(|dp| dp.volume())
    }

    fn datetimes(&self) -> impl Iterator<Item = DateTime<Utc>> + '_ {
        self.iter()
            .map(|dp| dp.datetime().expect("Invalid DataPoint datetime"))
    }

    fn timestamps(&self) -> impl Iterator<Item = i64> + '_ {
        self.iter().map(|dp| dp.timestamp())
    }
}

pub trait OHLCV {
    fn datetime(&self) -> Result<DateTime<Utc>>;
    fn timestamp(&self) -> i64;
    fn open(&self) -> f64;
    fn high(&self) -> f64;
    fn low(&self) -> f64;
    fn close(&self) -> f64;
    fn volume(&self) -> f64;
    fn is_ohlcv(&self) -> bool;
    fn is_ohlc4(&self) -> bool;

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
    fn datetime(&self) -> Result<DateTime<Utc>> {
        if self.value.is_empty() {
            return Err(Error::Generic("DataPoint value is empty".to_owned()));
        }

        let timestamp = self.value[0] as i64;
        if let Some(datetime) = DateTime::<Utc>::from_timestamp(timestamp, 0) {
            return Ok(datetime);
        }

        Err(Error::Generic(
            "Failed to convert timestamp to DateTime".to_owned(),
        ))
    }

    fn open(&self) -> f64 {
        if !(self.is_ohlcv() || self.is_ohlc4()) {
            return f64::NAN;
        }
        self.value[1]
    }

    fn high(&self) -> f64 {
        if !(self.is_ohlcv() || self.is_ohlc4()) {
            return f64::NAN;
        }
        self.value[2]
    }

    fn low(&self) -> f64 {
        if !(self.is_ohlcv() || self.is_ohlc4()) {
            return f64::NAN;
        }
        self.value[3]
    }

    fn close(&self) -> f64 {
        if !(self.is_ohlcv() || self.is_ohlc4()) {
            return f64::NAN;
        }
        self.value[4]
    }

    fn volume(&self) -> f64 {
        if !(self.is_ohlcv()) {
            return f64::NAN;
        }
        self.value[5]
    }

    fn is_ohlcv(&self) -> bool {
        self.value.len() == 6
    }

    fn is_ohlc4(&self) -> bool {
        self.value.len() == 5
    }

    fn timestamp(&self) -> i64 {
        self.value[0] as i64
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

#[cfg_attr(not(feature = "protobuf"), derive(Debug, Default))]
#[cfg_attr(feature = "protobuf", derive(prost::Message))]
#[derive(Clone, PartialEq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "camelCase")]
pub struct SymbolInfo {
    #[cfg_attr(feature = "protobuf", prost(string, tag = "1"))]
    #[serde(rename(deserialize = "pro_name"), default)]
    pub id: String,
    #[cfg_attr(feature = "protobuf", prost(string, tag = "2"))]
    #[serde(default)]
    pub name: String,
    #[cfg_attr(feature = "protobuf", prost(string, tag = "3"))]
    #[serde(default)]
    pub description: String,
    #[cfg_attr(feature = "protobuf", prost(string, tag = "4"))]
    pub exchange: String,
    #[cfg_attr(feature = "protobuf", prost(string, tag = "5"))]
    #[serde(rename = "listed_exchange", default)]
    pub listed_exchange: String,
    #[cfg_attr(feature = "protobuf", prost(string, tag = "6"))]
    #[serde(rename = "provider_id", default)]
    pub provider_id: String,
    #[cfg_attr(feature = "protobuf", prost(string, tag = "7"))]
    #[serde(rename = "base_currency", default)]
    pub base_currency: String,
    #[cfg_attr(feature = "protobuf", prost(string, tag = "8"))]
    #[serde(rename = "base_currency_id", default)]
    pub base_currency_id: String,
    #[cfg_attr(feature = "protobuf", prost(string, tag = "9"))]
    #[serde(rename = "currency_id", default)]
    pub currency_id: String,
    #[cfg_attr(feature = "protobuf", prost(string, tag = "10"))]
    #[serde(rename = "currency_code", default)]
    pub currency_code: String,
    #[cfg_attr(feature = "protobuf", prost(string, tag = "11"))]
    #[serde(default)]
    pub session_holidays: String,
    #[cfg_attr(feature = "protobuf", prost(message, repeated, tag = "12"))]
    #[serde(default)]
    pub subsessions: Vec<Subsession>,
    #[cfg_attr(feature = "protobuf", prost(string, tag = "13"))]
    #[serde(default)]
    pub timezone: String,
    #[cfg_attr(feature = "protobuf", prost(string, tag = "14"))]
    #[serde(rename(deserialize = "type"), default)]
    pub market_type: String,
    #[cfg_attr(feature = "protobuf", prost(string, repeated, tag = "15"))]
    #[serde(default)]
    pub typespecs: Vec<String>,
    #[cfg_attr(feature = "protobuf", prost(string, repeated, tag = "16"))]
    #[serde(default)]
    pub aliases: Vec<String>,
}

impl MarketSymbol for SymbolInfo {
    fn id(&self) -> String {
        self.id.clone()
    }

    fn symbol(&self) -> &str {
        let symbol = self.id.split(':').collect::<Vec<&str>>()[1];
        symbol
    }

    fn exchange(&self) -> &str {
        let exchange = self.id.split(':').collect::<Vec<&str>>()[0];
        exchange
    }

    fn currency(&self) -> &str {
        &self.currency_id
    }
}

#[cfg_attr(not(feature = "protobuf"), derive(Debug, Default))]
#[cfg_attr(feature = "protobuf", derive(prost::Message))]
#[derive(Clone, PartialEq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "camelCase")]
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
