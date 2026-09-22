//! Result container for study and fundamental data retrieval.

use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::chart::{DataPoint, SymbolInfo};

/// The final result of a study data retrieval.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StudyResult {
    /// Symbol metadata returned during symbol resolution.
    pub symbol_info: SymbolInfo,
    /// Collected and deduplicated data points for the study.
    pub data: Vec<DataPoint>,
    /// The unique study identifier used during retrieval.
    pub study_id: String,
    /// Total raw study data points received before deduplication.
    pub total_points_received: usize,
    /// Wall-clock duration elapsed during the retrieval.
    pub elapsed: Duration,
}

impl StudyResult {
    /// Number of data points in the result.
    #[inline]
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Whether any data points were received.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Slice of the collected study data points.
    #[inline]
    pub fn points(&self) -> &[DataPoint] {
        &self.data
    }

    /// Mutable slice of the collected study data points.
    #[inline]
    pub fn points_mut(&mut self) -> &mut [DataPoint] {
        &mut self.data
    }

    /// Consumes the result and returns the inner vector of data points.
    #[inline]
    pub fn into_points(self) -> Vec<DataPoint> {
        self.data
    }

    /// Timestamp of the first data point, if available.
    ///
    /// Extracted directly from `value[0]` without assuming OHLCV structure.
    #[inline]
    pub fn first_timestamp(&self) -> Option<i64> {
        self.data
            .first()
            .and_then(|dp| dp.value.first().copied().map(|v| v as i64))
    }

    /// Timestamp of the last data point, if available.
    ///
    /// Extracted directly from `value[0]` without assuming OHLCV structure.
    #[inline]
    pub fn last_timestamp(&self) -> Option<i64> {
        self.data
            .last()
            .and_then(|dp| dp.value.first().copied().map(|v| v as i64))
    }

    /// UTC DateTime of the first data point, if available.
    #[inline]
    pub fn first_datetime(&self) -> Option<DateTime<Utc>> {
        self.first_timestamp()
            .and_then(|ts| DateTime::<Utc>::from_timestamp(ts, 0))
    }

    /// UTC DateTime of the last data point, if available.
    #[inline]
    pub fn last_datetime(&self) -> Option<DateTime<Utc>> {
        self.last_timestamp()
            .and_then(|ts| DateTime::<Utc>::from_timestamp(ts, 0))
    }
}
