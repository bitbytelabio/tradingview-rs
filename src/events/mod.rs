//! Generic market event types.
//!
//! This module defines [`MarketEvent`], a non-exhaustive enum that represents
//! any kind of market data that the loader can produce. New event variants can
//! be added without breaking existing consumers.

pub mod models;

pub use models::*;
