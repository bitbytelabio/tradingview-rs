use std::sync::Arc;
use std::time::Instant;

use crate::websocket::SeriesInfo;
use crate::{DataPoint, OHLCV, SymbolInfo};

#[derive(Debug)]
pub struct HistoricalState {
    pub data: Vec<DataPoint>,
    pub symbol_info: Option<SymbolInfo>,
    pub series_info: Option<SeriesInfo>,
    pub replay: ReplayState,
    pub error_count: u32,
    pub completed: bool,
    pub errored: bool,
    pub error_message: Option<String>,
    pub first_data_at: Option<Instant>,
    pub total_bars: usize,
    pub notify: Arc<tokio::sync::Notify>,
}

impl HistoricalState {
    pub fn new() -> Self {
        Self::with_notify(Arc::new(tokio::sync::Notify::new()))
    }
    pub fn with_notify(notify: Arc<tokio::sync::Notify>) -> Self {
        Self {
            data: Vec::new(),
            symbol_info: None,
            series_info: None,
            replay: ReplayState::default(),
            error_count: 0,
            completed: false,
            errored: false,
            error_message: None,
            first_data_at: None,
            total_bars: 0,
            notify,
        }
    }
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            data: Vec::with_capacity(capacity),
            ..Self::new()
        }
    }
    pub fn with_capacity_and_notify(capacity: usize, notify: Arc<tokio::sync::Notify>) -> Self {
        Self {
            data: Vec::with_capacity(capacity),
            ..Self::with_notify(notify)
        }
    }
    pub fn record_chart_data(&mut self, series_info: SeriesInfo, points: Vec<DataPoint>) {
        self.series_info = Some(series_info);
        let received = points.len();
        if self.first_data_at.is_none() {
            self.first_data_at = Some(Instant::now());
        }
        self.data.extend(points);
        self.total_bars += received;
    }
    pub fn record_points(&mut self, points: Vec<DataPoint>, total_in_batch: usize) {
        if self.first_data_at.is_none() {
            self.first_data_at = Some(Instant::now());
        }
        self.data.extend(points);
        self.total_bars += total_in_batch;
    }
    pub fn record_symbol_info(&mut self, info: SymbolInfo) {
        self.symbol_info = Some(info);
    }
    pub fn record_error(&mut self) -> bool {
        self.error_count += 1;
        self.error_count > 5
    }
    pub fn complete(&mut self) {
        self.completed = true;
        self.notify.notify_waiters();
    }
    pub fn fail(&mut self, msg: String) {
        self.errored = true;
        self.error_message = Some(msg);
        self.notify.notify_waiters();
    }
    pub fn is_done(&self) -> bool {
        self.completed || self.errored
    }
    pub fn finalize(&mut self) -> Vec<DataPoint> {
        self.data.sort_by_key(|a| a.timestamp());
        self.data.dedup_by_key(|p| p.timestamp());
        std::mem::take(&mut self.data)
    }
}
impl Default for HistoricalState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Default)]
pub struct ReplayState {
    pub enabled: bool,
    pub configured: bool,
    pub earliest_ts: Option<i64>,
    pub data_received: bool,
}
impl ReplayState {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            ..Default::default()
        }
    }
    pub fn update_earliest(&mut self, ts: i64) {
        self.earliest_ts = Some(self.earliest_ts.map(|e| e.min(ts)).unwrap_or(ts));
    }
    pub fn needs_setup(&self) -> bool {
        self.enabled && !self.configured && self.data_received
    }
}
