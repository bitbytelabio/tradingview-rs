use serde::{Deserialize, Serialize};

use crate::{DataPoint, Interval};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OHLCV {
    pub timestamp: u64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChartHistoricalData {
    pub symbol: String,
    pub exchange: String,
    pub interval: Interval,
    pub data: Vec<OHLCV>,
}

impl ChartHistoricalData {
    pub fn new(symbol: &str, exchange: &str, interval: Interval) -> Self {
        Self {
            symbol: symbol.to_string(),
            exchange: exchange.to_string(),
            interval,
            data: Vec::new(),
        }
    }

    pub fn add_points(&mut self, points: Vec<DataPoint>) {
        for p in points {
            self.data.push(OHLCV {
                timestamp: p.value[0] as u64,
                open: p.value[1],
                high: p.value[2],
                low: p.value[3],
                close: p.value[4],
                volume: p.value[5],
            });
        }
    }
}
