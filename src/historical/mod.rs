//! Historical data retrieval — clean architecture for fetching TradingView
//! chart data via WebSocket.
//!
//! # Architecture
//!
//! ```text
//! HistoricalClient::retrieve(request)
//!   ├── HistoricalRequest  (what to fetch)
//!   ├── HistoricalState    (mutable state during fetch)
//!   └── returns HistoricalResult
//!
//! HistoricalClient::retrieve_batch(symbols, interval, num_bars, config)
//!   ├── Runs N concurrent HistoricalClient::retrieve() calls
//!   ├── Semaphore-controlled concurrency
//!   └── returns BatchResult (successful + failed)
//! ```
//!
//! # Migration from v1
//!
//! The old `retrieve()` function in `single.rs` used scattered
//! `Arc<Mutex<...>>` fields and unbounded channels.  The new
//! `HistoricalClient` consolidates state, uses bounded channels, and
//! provides a builder-based `HistoricalRequest`.
//!
//! The old batch implementation used a complex `CommandRunner` +
//! `BatchTracker` + `DataRx` architecture.  The new `retrieve_batch()`
//! uses `JoinSet` + `Semaphore` for dramatically simpler concurrent
//! fetching.

pub mod batch;
pub mod client;
pub mod request;
pub mod result;
pub mod state;

// Re-export the public API.
pub use batch::{BatchConfig, BatchResult, BatchSymbolResult};
pub use client::HistoricalClient;
pub use request::HistoricalRequest;
pub use result::HistoricalResult;
