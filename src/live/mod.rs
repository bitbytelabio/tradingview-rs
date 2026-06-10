//! Real-time market data via TradingView WebSocket connections.
//!
//! This module provides live streaming of chart data, quotes, and studies
//! through persistent WebSocket sessions.
//!
//! # Architecture
//!
//! ```text
//! WebSocketClient ──► data_tx (channel) ──► your code
//!        ▲
//!        │  commands
//!        │
//! CommandRunner ─── command_rx (channel) ─── your code
//! ```
//!
//! - **[`websocket`]** — Low-level WebSocket client for connecting to
//!   TradingView data servers.
//! - **[`handler`]** — Message parsing, command dispatch, and response routing.
//! - **[`models`]** — Data types for live data (quotes, chart updates, studies).

pub mod handler;
pub mod models;
pub mod websocket;
