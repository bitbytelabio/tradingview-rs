//! HTTP client layer for TradingView's REST API endpoints.
//!
//! This module provides type-safe access to TradingView's non-WebSocket APIs:
//!
//! - **[`misc`]** — Symbol search, market status, symbol info, and other
//!   miscellaneous queries.
//! - **[`news`]** — TradingView news feed and headline retrieval.
//! - **[`fin_calendar`]** — Financial calendar (earnings, dividends, economic
//!   events).
//! - **[`core`]** — Shared [`DataClient`] trait for HTTP client implementations.
//!
//! All HTTP calls are async and return [`crate::Result<T>`].
//!
//! [`DataClient`]: core::DataClient

pub mod core;
pub mod fin_calendar;
pub mod misc;
pub mod news;
