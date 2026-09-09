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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SeriesDataResponse {
    String(Ustr),
    ChartResponseData(ChartResponseData),
    SymbolInfo(SymbolInfo),
    StudyResponseData(StudyResponseData),
    JsonValue(Value),
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
        self.data.iter()
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ChartResponseData {
    #[serde(default)]
    pub node: Option<Ustr>,
    #[serde(rename(deserialize = "s"))]
    pub series: Vec<DataPoint>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct StudyResponseData {
    #[serde(default)]
    pub node: Option<Ustr>,
    #[serde(rename(deserialize = "st"))]
    pub studies: Vec<DataPoint>,
    #[serde(rename(deserialize = "ns"))]
    pub raw_graphics: GraphicDataResponse,
}

// TODO: Implement graphic parser for indexes response
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
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
        let ts = self.timestamp();
        DateTime::<Utc>::from_timestamp(ts, 0).unwrap_or_default()
    }

    fn timestamp(&self) -> i64 {
        self.value.first().copied().map(|v| v as i64).unwrap_or(0)
    }

    fn open(&self) -> f64 {
        if self.value.len() < 5 {
            return f64::NAN;
        }
        self.value[1]
    }

    fn high(&self) -> f64 {
        if self.value.len() < 5 {
            return f64::NAN;
        }
        self.value[2]
    }

    fn low(&self) -> f64 {
        if self.value.len() < 5 {
            return f64::NAN;
        }
        self.value[3]
    }

    fn close(&self) -> f64 {
        if self.value.len() < 5 {
            return f64::NAN;
        }
        self.value[4]
    }

    fn volume(&self) -> f64 {
        if self.value.len() < 6 {
            return f64::NAN;
        }
        self.value[5]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartDataChanges {
    pub changes: Vec<f64>,
    pub index: i64,
    pub index_diff: Vec<Value>,
    pub marks: Vec<Value>,
    pub zoffset: i64,
}

#[derive(Clone, PartialEq, Serialize, Deserialize, Hash, Debug, Default, Copy)]
pub struct SeriesCompletedMessage {
    #[serde(default)]
    pub id: Ustr,
    #[serde(default)]
    pub update_mode: Ustr,
}

#[derive(Clone, PartialEq, Serialize, Deserialize, Debug, Default, Builder, Copy)]
pub struct Ticker {
    pub symbol: Ustr,
    pub exchange: Ustr,
    pub currency: Option<Currency>,
    pub country: Option<Currency>,
    pub market_type: Option<MarketType>,
}

impl Ticker {
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

impl MarketSymbol for Ticker {
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

impl From<&SymbolInfo> for Ticker {
    fn from(symbol_info: &SymbolInfo) -> Self {
        Self {
            symbol: symbol_info.name,
            exchange: symbol_info.exchange,
            currency: Currency::from_code(symbol_info.currency_id.as_str()),
            country: None,
            market_type: Some(MarketType::from(symbol_info.market_type.as_str())),
        }
    }
}

impl From<SymbolInfo> for Ticker {
    fn from(val: SymbolInfo) -> Self {
        Ticker::from(&val)
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build a valid OHLCV `DataPoint` from raw values.
    /// Layout: [timestamp, open, high, low, close, volume]
    fn dp(ts: i64, o: f64, h: f64, l: f64, c: f64, v: f64) -> DataPoint {
        DataPoint {
            index: 0,
            value: vec![ts as f64, o, h, l, c, v],
        }
    }

    /// Build a vector of `N` consecutive daily candles starting at `base_ts`.
    fn make_candles(n: usize) -> Vec<DataPoint> {
        let base_ts = 1_700_000_000i64; // Nov 2023
        (0..n)
            .map(|i| {
                let ts = base_ts + i as i64 * 86_400;
                let o = 100.0 + i as f64;
                let c = 101.0 + i as f64;
                dp(ts, o, o + 1.0, o - 1.0, c, 1000.0 + i as f64)
            })
            .collect()
    }

    /// Helper: build a `ChartHistoricalData` with the given candles.
    /// Constructs a valid `ChartOptions` to satisfy the `SeriesInfo` invariant.
    fn make_chart(data: Vec<DataPoint>) -> ChartHistoricalData {
        let options = crate::chart::ChartOptions::builder()
            .instrument("NASDAQ:AAPL")
            .build()
            .expect("valid ChartOptions for testing");
        ChartHistoricalData {
            symbol_info: SymbolInfo::default(),
            series_info: SeriesInfo {
                chart_session: Ustr::default(),
                options,
            },
            data,
        }
    }

    // ------------------------------------------------------------------
    // Vec<DataPoint> — PriceIterable
    // ------------------------------------------------------------------

    #[test]
    fn test_vec_to_vec_returns_references() {
        let candles = make_candles(5);
        let collected: Vec<&DataPoint> = candles.to_vec().collect();
        assert_eq!(collected.len(), 5);
        // The original Vec is still usable — we only borrowed.
        assert_eq!(candles.len(), 5);
    }

    #[test]
    fn test_vec_closes() {
        let candles = make_candles(3);
        let closes: Vec<f64> = candles.closes().collect();
        assert_eq!(closes, vec![101.0, 102.0, 103.0]);
    }

    #[test]
    fn test_vec_opens() {
        let candles = make_candles(3);
        let opens: Vec<f64> = candles.opens().collect();
        assert_eq!(opens, vec![100.0, 101.0, 102.0]);
    }

    #[test]
    fn test_vec_highs() {
        let candles = make_candles(3);
        let highs: Vec<f64> = candles.highs().collect();
        assert_eq!(highs, vec![101.0, 102.0, 103.0]);
    }

    #[test]
    fn test_vec_lows() {
        let candles = make_candles(3);
        let lows: Vec<f64> = candles.lows().collect();
        assert_eq!(lows, vec![99.0, 100.0, 101.0]);
    }

    #[test]
    fn test_vec_volumes() {
        let candles = make_candles(3);
        let volumes: Vec<f64> = candles.volumes().collect();
        assert_eq!(volumes, vec![1000.0, 1001.0, 1002.0]);
    }

    #[test]
    fn test_vec_timestamps() {
        let candles = make_candles(3);
        let base = 1_700_000_000i64;
        let timestamps: Vec<i64> = candles.timestamps().collect();
        assert_eq!(timestamps, vec![base, base + 86_400, base + 2 * 86_400]);
    }

    #[test]
    fn test_vec_datetimes() {
        let candles = make_candles(2);
        let base = 1_700_000_000i64;
        let datetimes: Vec<DateTime<Utc>> = candles.datetimes().collect();
        assert_eq!(datetimes.len(), 2);
        // First candle timestamp
        let expected = DateTime::<Utc>::from_timestamp(base, 0).unwrap();
        assert_eq!(datetimes[0], expected);
    }

    // ------------------------------------------------------------------
    // ChartHistoricalData — PriceIterable
    // ------------------------------------------------------------------

    #[test]
    fn test_chart_historical_to_vec_returns_references() {
        let candles = make_candles(5);
        let chart = make_chart(candles.clone());
        let collected: Vec<&DataPoint> = chart.to_vec().collect();
        assert_eq!(collected.len(), 5);
        // Original chart still usable — to_vec() borrows.
        assert_eq!(chart.data.len(), 5);
    }

    #[test]
    fn test_chart_historical_closes() {
        let candles = make_candles(3);
        let chart = make_chart(candles);
        let closes: Vec<f64> = chart.closes().collect();
        assert_eq!(closes, vec![101.0, 102.0, 103.0]);
    }

    #[test]
    fn test_chart_historical_opens() {
        let candles = make_candles(3);
        let chart = make_chart(candles);
        let opens: Vec<f64> = chart.opens().collect();
        assert_eq!(opens, vec![100.0, 101.0, 102.0]);
    }

    #[test]
    fn test_chart_historical_highs() {
        let candles = make_candles(3);
        let chart = make_chart(candles);
        let highs: Vec<f64> = chart.highs().collect();
        assert_eq!(highs, vec![101.0, 102.0, 103.0]);
    }

    #[test]
    fn test_chart_historical_lows() {
        let candles = make_candles(3);
        let chart = make_chart(candles);
        let lows: Vec<f64> = chart.lows().collect();
        assert_eq!(lows, vec![99.0, 100.0, 101.0]);
    }

    #[test]
    fn test_chart_historical_volumes() {
        let candles = make_candles(3);
        let chart = make_chart(candles);
        let volumes: Vec<f64> = chart.volumes().collect();
        assert_eq!(volumes, vec![1000.0, 1001.0, 1002.0]);
    }

    #[test]
    fn test_chart_historical_timestamps() {
        let candles = make_candles(3);
        let chart = make_chart(candles);
        let base = 1_700_000_000i64;
        let timestamps: Vec<i64> = chart.timestamps().collect();
        assert_eq!(timestamps, vec![base, base + 86_400, base + 2 * 86_400]);
    }

    #[test]
    fn test_empty_vec_no_panic() {
        let empty: Vec<DataPoint> = vec![];
        // All iterator methods should work on empty data.
        assert_eq!(empty.to_vec().count(), 0);
        assert_eq!(empty.closes().count(), 0);
        assert_eq!(empty.opens().count(), 0);
        assert_eq!(empty.highs().count(), 0);
        assert_eq!(empty.lows().count(), 0);
        assert_eq!(empty.volumes().count(), 0);
        assert_eq!(empty.timestamps().count(), 0);
        assert_eq!(empty.datetimes().count(), 0);
    }

    #[test]
    fn test_empty_chart_no_panic() {
        let chart = make_chart(vec![]);
        assert_eq!(chart.to_vec().count(), 0);
        assert_eq!(chart.closes().count(), 0);
        assert_eq!(chart.opens().count(), 0);
        assert_eq!(chart.highs().count(), 0);
        assert_eq!(chart.lows().count(), 0);
        assert_eq!(chart.volumes().count(), 0);
        assert_eq!(chart.timestamps().count(), 0);
        assert_eq!(chart.datetimes().count(), 0);
    }

    // ------------------------------------------------------------------
    // Consistency: both impls must produce the same output for the same data
    // ------------------------------------------------------------------

    #[test]
    fn test_vec_and_chart_produce_same_closes() {
        let candles = make_candles(50);
        let chart = make_chart(candles.clone());
        let from_vec: Vec<f64> = candles.closes().collect();
        let from_chart: Vec<f64> = chart.closes().collect();
        assert_eq!(from_vec, from_chart);
    }

    #[test]
    fn test_vec_and_chart_produce_same_opens() {
        let candles = make_candles(50);
        let chart = make_chart(candles.clone());
        let from_vec: Vec<f64> = candles.opens().collect();
        let from_chart: Vec<f64> = chart.opens().collect();
        assert_eq!(from_vec, from_chart);
    }

    #[test]
    fn test_vec_and_chart_produce_same_timestamps() {
        let candles = make_candles(50);
        let chart = make_chart(candles.clone());
        let from_vec: Vec<i64> = candles.timestamps().collect();
        let from_chart: Vec<i64> = chart.timestamps().collect();
        assert_eq!(from_vec, from_chart);
    }

    // ------------------------------------------------------------------
    // OHLCV trait methods
    // ------------------------------------------------------------------

    #[test]
    fn test_ohlcv_validate_rejects_invalid_high_low() {
        // high < low
        let bad = dp(1_700_000_000, 100.0, 99.0, 100.0, 100.5, 1000.0);
        assert!(!bad.validate());
    }

    #[test]
    fn test_ohlcv_validate_rejects_close_above_high() {
        let bad = dp(1_700_000_000, 100.0, 105.0, 99.0, 106.0, 1000.0);
        assert!(!bad.validate());
    }

    #[test]
    fn test_ohlcv_validate_rejects_close_below_low() {
        let bad = dp(1_700_000_000, 100.0, 105.0, 99.0, 98.0, 1000.0);
        assert!(!bad.validate());
    }

    #[test]
    fn test_ohlcv_validate_accepts_valid_candle() {
        let good = dp(1_700_000_000, 100.0, 105.0, 99.0, 102.0, 1000.0);
        assert!(good.validate());
    }

    #[test]
    fn test_ohlcv_tr() {
        let prev = dp(1_700_000_000, 100.0, 105.0, 99.0, 102.0, 1000.0);
        let curr = dp(1_700_086_400, 101.0, 108.0, 100.0, 104.0, 1100.0);
        // TR = max(high, prev_close) - min(low, prev_close)
        // = max(108, 102) - min(100, 102) = 108 - 100 = 8
        let tr = curr.tr(&prev);
        assert!((tr - 8.0).abs() < 1e-10);
    }

    #[test]
    fn test_ohlcv_is_rising() {
        let rising = dp(1_700_000_000, 100.0, 105.0, 99.0, 102.0, 1000.0);
        assert!(rising.is_rising());
    }

    #[test]
    fn test_ohlcv_is_falling() {
        let falling = dp(1_700_000_000, 102.0, 105.0, 99.0, 100.0, 1000.0);
        assert!(falling.is_falling());
    }
}
