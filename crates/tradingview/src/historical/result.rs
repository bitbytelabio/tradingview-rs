use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::chart::OHLCV;
use crate::websocket::SeriesInfo;
use crate::{DataPoint, SymbolInfo};

/// The final result of a historical data retrieval.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoricalResult {
    pub symbol_info: SymbolInfo,
    pub data: Vec<DataPoint>,
    pub series_info: Option<SeriesInfo>,
    pub total_bars_received: usize,
    pub replay_used: bool,
    pub elapsed: std::time::Duration,
}

impl HistoricalResult {
    pub fn len(&self) -> usize {
        self.data.len()
    }
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
    pub fn first_timestamp(&self) -> Option<i64> {
        self.data.first().map(|dp| dp.timestamp())
    }
    pub fn last_timestamp(&self) -> Option<i64> {
        self.data.last().map(|dp| dp.timestamp())
    }
    pub fn first_datetime(&self) -> Option<DateTime<Utc>> {
        self.data.first().map(|dp| dp.datetime())
    }
    pub fn last_datetime(&self) -> Option<DateTime<Utc>> {
        self.data.last().map(|dp| dp.datetime())
    }
}
