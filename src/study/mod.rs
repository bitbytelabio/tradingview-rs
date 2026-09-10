//! TradingView study and fundamental data retrieval.
//!
//! Provides a clean, high-level API modeled after [`crate::historical::HistoricalClient`] for fetching
//! TradingView indicator studies (both built-in and Pine Script studies) over WebSocket.
//!
//! # Architecture
//!
//! ```text
//! StudyClient::retrieve(request)
//!   ├── StudyRequest       (symbol/exchange, study config, interval, bar count, timeout)
//!   ├── StudyState         (accumulates points keyed by study ID, tracks completion)
//!   ├── StudyDataHandler   (inspects timescale_update / du for study points)
//!   └── returns StudyResult (symbol info, sorted/deduplicated points, elapsed duration)
//! ```
//!
//! # Example
//!
//! ```no_run
//! use tradingview::Interval;
//! use tradingview::chart::study::StudyConfiguration;
//! use tradingview::study::{StudyClient, StudyRequest};
//! use tradingview::models::pine_indicator::{PineIndicator, ScriptType};
//! use tradingview::DataServer;
//!
//! # async fn run() -> tradingview::Result<()> {
//! let client = StudyClient::new("unauthorized_user_token", DataServer::Data);
//!
//! let indicator = PineIndicator::build()
//!     .fetch("STD;Fund_total_revenue_fy", "69.0", ScriptType::IntervalScript)
//!     .await?;
//!
//! let request = StudyRequest::builder()
//!     .symbol("AAPL")
//!     .exchange("NASDAQ")
//!     .interval(Interval::OneDay)
//!     .base_bar_count(100)
//!     .study(StudyConfiguration::Pine(Box::new(indicator)))
//!     .build();
//!
//! let result = client.retrieve(request).await?;
//! println!("received {} study points for {}", result.len(), result.symbol_info.name);
//! # Ok(())
//! # }
//! ```

mod client;
mod request;
mod result;
mod state;

pub use crate::chart::study::StudyConfiguration;
pub use client::StudyClient;
pub use request::StudyRequest;
pub use result::StudyResult;
