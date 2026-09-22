//! Mutable state accumulator for study data retrieval.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::Notify;

use crate::chart::{DataPoint, SymbolInfo};

/// Mutable state accumulated across incoming WebSocket frames during a study retrieval.
#[derive(Debug)]
pub struct StudyState {
    /// Accumulated study data points.
    pub data: Vec<DataPoint>,
    /// Symbol info received from `resolve_symbol`.
    pub symbol_info: Option<SymbolInfo>,
    /// Transient error counter for fail-threshold tracking.
    pub error_count: u32,
    /// Whether `study_completed` was received for this study.
    pub completed: bool,
    /// Whether an unrecoverable or protocol error occurred.
    pub errored: bool,
    /// Detailed error message if `errored` is true.
    pub error_message: Option<String>,
    /// Timestamp when the first data chunk was recorded.
    pub first_data_at: Option<Instant>,
    /// Total count of study points successfully parsed across all chunks before dedup.
    pub total_points: usize,
    /// Notification mechanism to wake the client when finished.
    pub notify: Arc<Notify>,
}

impl StudyState {
    /// Creates a new state.
    #[cfg(test)]
    pub fn new() -> Self {
        Self::with_notify(Arc::new(Notify::new()))
    }

    /// Creates a new state with a pre-configured [`Notify`] instance.
    pub fn with_notify(notify: Arc<Notify>) -> Self {
        Self {
            data: Vec::new(),
            symbol_info: None,
            error_count: 0,
            completed: false,
            errored: false,
            error_message: None,
            first_data_at: None,
            total_points: 0,
            notify,
        }
    }

    /// Creates a new state with initial capacity and a pre-configured [`Notify`].
    pub fn with_capacity_and_notify(capacity: usize, notify: Arc<Notify>) -> Self {
        Self {
            data: Vec::with_capacity(capacity),
            ..Self::with_notify(notify)
        }
    }

    /// Records a batch of data points received for this study.
    pub fn record_points(&mut self, points: Vec<DataPoint>, count: usize) {
        if self.first_data_at.is_none() {
            self.first_data_at = Some(Instant::now());
        }
        self.data.extend(points);
        self.total_points += count;
    }

    /// Records resolved symbol information.
    pub fn record_symbol_info(&mut self, info: SymbolInfo) {
        self.symbol_info = Some(info);
    }

    /// Increments the error count and returns true if threshold is exceeded.
    pub fn record_error(&mut self) -> bool {
        self.error_count += 1;
        self.error_count > 5
    }

    /// Marks the study retrieval as successfully completed.
    pub fn complete(&mut self) {
        self.completed = true;
        self.notify.notify_waiters();
    }

    /// Marks the study retrieval as failed with an error message.
    pub fn fail(&mut self, msg: String) {
        self.errored = true;
        self.error_message = Some(msg);
        self.notify.notify_waiters();
    }

    /// Returns whether retrieval has concluded (either completed or errored).
    #[inline]
    pub fn is_done(&self) -> bool {
        self.completed || self.errored
    }

    /// Sorts and deduplicates points by timestamp/index without treating study values as OHLCV,
    /// replacing earlier entries with the latest-arrived point for each logical key.
    pub fn finalize(&mut self) -> Vec<DataPoint> {
        // Study points have values: [timestamp, plot0, plot1, ...].
        // They are NOT OHLCV bars.
        // Dedup semantics: later points for the same logical timestamp/index are updates
        // and replace earlier points. We use a BTreeMap so that:
        // 1. Later points overwrite earlier ones with the exact same key.
        // 2. Entries are deterministically sorted by ascending (timestamp, index).
        let mut map = BTreeMap::new();
        for dp in self.data.drain(..) {
            let ts = dp.value.first().copied().map(|v| v as i64).unwrap_or(0);
            let key = if ts != 0 {
                (ts, dp.index)
            } else {
                (0, dp.index)
            };
            map.insert(key, dp);
        }
        map.into_values().collect()
    }
}
