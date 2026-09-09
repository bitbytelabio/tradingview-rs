//! Live quote streaming example.
//!
//! Demonstrates how to stream real-time quote data from TradingView using
//! the `Handler` (v2) trait and `WebSocketClient` directly — no
//! `CommandRunner` or legacy channel plumbing needed.
//!
//! # Usage
//!
//! Set `TV_AUTH_TOKEN` in your environment or a `.env` file, then run:
//!
//! ```bash
//! cargo run --example live_quote
//! ```
//!
//! Press Ctrl-C to stop.  The example streams quotes indefinitely and
//! logs each tick to the console.

use std::sync::Arc;

use serde_json::Value;
use tokio::signal;
use tracing::{debug, error, info, warn};

use tradingview::{
    DataServer, Error,
    live::{handler::Handler, models::TradingViewDataEvent, websocket::WebSocketClient},
};

// =============================================================================
// LiveQuoteHandler — minimal Handler implementation for live quotes
// =============================================================================

/// A simple handler that logs incoming TradingView data events.
///
/// Implements the [`Handler`] (v2) trait for use with [`WebSocketClient`].
/// For production use, replace the `tracing` calls with your own logic
/// (e.g., pushing to a channel, writing to a database, etc.).
struct LiveQuoteHandler;

impl Handler for LiveQuoteHandler {
    fn handle_events(&self, event: TradingViewDataEvent, message: &[Value]) {
        match event {
            TradingViewDataEvent::OnQuoteData => {
                info!(?message, "Quote data received");
            }
            TradingViewDataEvent::OnQuoteCompleted => {
                info!("Quote stream completed");
            }
            _ => {
                debug!(?event, ?message, "Event received");
            }
        }
    }

    fn handle_quote_data(&self, message: &[Value]) {
        // Quote data from the legacy quote data path.
        info!(?message, "Quote data (legacy path)");
    }

    fn handle_series_data(&self, _event: TradingViewDataEvent, _messages: &[Value]) {
        // Not used for pure quote streaming.
    }

    fn notify_error(&self, error: Error, message: &[Value]) {
        warn!(?error, ?message, "WebSocket error");
    }
}

// =============================================================================
// main
// =============================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load .env file if present, then initialise tracing.
    let _ = dotenv::dotenv();
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    // ── Configuration ──────────────────────────────────────────────────
    let auth_token = std::env::var("TV_AUTH_TOKEN").unwrap_or_else(|_| {
        eprintln!("TV_AUTH_TOKEN not set — using anonymous access (may fail)");
        "unauthorized_user_token".to_string()
    });

    let symbols: Vec<&str> = vec!["OKX:BTCUSDT.P", "BINANCE:ETHUSDT"];

    // ── Build WebSocket client with our handler ────────────────────────
    info!("Connecting to TradingView data feed...");
    let ws = WebSocketClient::builder()
        .auth_token(&auth_token)
        .server(DataServer::Data)
        .handler(LiveQuoteHandler)
        .build()
        .await?;

    // Start the reader task — this drives the event loop.
    Arc::clone(&ws).spawn_reader_task();

    // ── Create quote session and subscribe to symbols ──────────────────
    let quote_session = tradingview::utils::gen_session_id("qs");

    ws.create_quote_session(&quote_session).await?;
    info!(session = %quote_session, "Quote session created");

    ws.set_fields(&quote_session).await?;
    info!("Quote fields set");

    ws.add_symbols(&quote_session, &symbols).await?;
    info!(?symbols, "Subscribed to symbols");

    // Optionally request fast symbols for lower latency updates.
    // ws.fast_symbols(&quote_session, &symbols).await?;

    println!();
    println!("Streaming live quotes for: {symbols:?}");
    println!("Press Ctrl-C to stop.");
    println!();

    // ── Run until Ctrl-C ───────────────────────────────────────────────
    signal::ctrl_c().await?;
    info!("Shutting down...");

    // Clean up the quote session.
    if let Err(e) = ws.delete_quote_session(&quote_session).await {
        error!("Failed to delete quote session: {e}");
    }

    ws.close().await?;
    info!("Done.");

    Ok(())
}
