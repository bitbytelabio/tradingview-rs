# Rust Core API Reference (`tradingview-rs`)

`tradingview-rs` is an asynchronous data source library for algorithmic trading written in Rust.

---

## Workspace Layout

- Root: Virtual Cargo workspace managing members.
- `crates/tradingview`: Core Rust library crate (`tradingview-rs`).
- `crates/tradingview-py`: PyO3 0.29 native Python extension (`tradingview-py`).

---

## Two-Tier Architecture

1. **High-Level Event Pipeline (`DataLoader`)**:
   - Assembles a `DataSource` into multiple `EventSink`s (Channel, Callback, Kafka).
   - Handles fan-out, buffer bounds, backpressure, and graceful task lifecycle.
2. **Low-Level Protocol Primitives**:
   - `WebSocketClient`: Direct WebSocket connection with automatic exponential-backoff reconnect and circuit breaking.
   - `HistoricalClient`: Single and batch historical OHLCV chart retrieval.
   - `StudyClient`: Pine indicator execution over WebSocket.
   - `fin_calendar`: REST client for global macroeconomic events.
   - `fundamental`: Built-in Pine study catalog and date-versioned registry.

---

## Historical Data Retrieval

```rust
use tradingview::historical::{BatchConfig, HistoricalClient, HistoricalRequest};
use tradingview::live::models::DataServer;
use tradingview::models::Interval;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = HistoricalClient::new("unauthorized_user_token", DataServer::Data);

    // Single request builder
    let request = HistoricalRequest::builder()
        .symbol("AAPL")
        .exchange("NASDAQ")
        .interval(Interval::OneDay)
        .num_bars(100)
        .build();

    let result = client.retrieve(request).await?;
    println!("Bars received: {}", result.len());

    // Batch retrieval with semaphore-bounded concurrency
    let symbols = vec![
        ("AAPL".to_string(), "NASDAQ".to_string()),
        ("MSFT".to_string(), "NASDAQ".to_string()),
    ];

    let batch = client
        .retrieve_batch(
            &symbols,
            Interval::OneDay,
            Some(50),
            BatchConfig {
                max_concurrency: 4,
                per_symbol_timeout: std::time::Duration::from_secs(30),
            },
        )
        .await;

    println!("Batch successful: {}", batch.successful.len());
    Ok(())
}
```

---

## WebSocket & Quote Streaming

```rust
use serde_json::Value;
use std::sync::Arc;
use tradingview::live::{handler::Handler, models::TradingViewDataEvent, websocket::WebSocketClient};
use tradingview::{DataServer, Error};

struct MyHandler;

impl Handler for MyHandler {
    fn handle_events(&self, event: TradingViewDataEvent, message: &[Value]) {
        match event {
            TradingViewDataEvent::OnQuoteData => {
                // message[0]: session ID
                // message[1]: quote object {"n": "...", "v": {...}}
            }
            TradingViewDataEvent::OnChartData | TradingViewDataEvent::OnChartDataUpdate => {
                // Candlestick update
            }
            _ => {}
        }
    }

    fn handle_quote_data(&self, message: &[Value]) {}
    fn handle_series_data(&self, _event: TradingViewDataEvent, _messages: &[Value]) {}
    fn notify_error(&self, error: Error, message: &[Value]) {
        eprintln!("Error: {:?}, payload: {:?}", error, message);
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ws = WebSocketClient::builder()
        .auth_token("unauthorized_user_token")
        .server(DataServer::Data)
        .handler(MyHandler)
        .build()
        .await?;

    Arc::clone(&ws).spawn_reader_task();

    let qs = tradingview::utils::gen_session_id("qs");
    ws.create_quote_session(&qs).await?;
    ws.set_fields(&qs).await?;
    ws.add_symbols(&qs, &["BINANCE:BTCUSDT"]).await?;

    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

    ws.delete_quote_session(&qs).await?;
    ws.close().await?;
    Ok(())
}
```

---

## Corporate Fundamentals & Economic Calendar

```rust
use tradingview::client::fin_calendar::{EconomicCalendarRequest, EconomicImportance, get_economic_calendar};
use tradingview::fundamental::{fetch_fundamental_registry, get_fundamental_data};
use tradingview::models::{FinancialPeriod, Interval};
use tradingview::live::models::DataServer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Fundamentals via Registry
    let registry = fetch_fundamental_registry().await?;
    let study = get_fundamental_data(
        &registry,
        "total_revenue",
        Some(&FinancialPeriod::FiscalYear),
        "AAPL",
        "NASDAQ",
        Interval::OneDay,
        5,
        Some("unauthorized_user_token"),
        DataServer::Data,
    )
    .await?;
    println!("Fundamental points received: {}", study.len());

    // 2. Macroeconomic Calendar
    let request = EconomicCalendarRequest::builder()
        .from(chrono::Utc::now())
        .to(chrono::Utc::now() + chrono::Duration::days(7))
        .countries(vec!["US".to_string()])
        .min_importance(EconomicImportance::High)
        .build();

    let events = get_economic_calendar(&request).await?;
    println!("High-importance US events: {}", events.len());

    Ok(())
}
```

---

## Wire Protocol & Frame Encoding

- **Framing**: Packets follow the `~m~<length>~m~<payload>` framing structure.
- **Length Encoding**: The payload length MUST be counted in **UTF-16 code units** (matching JavaScript string length semantics). Multi-byte UTF-8 sequences (such as Vietnamese diacritics, CJK characters, or emojis) contain fewer UTF-16 code units than raw UTF-8 bytes. `tradingview-rs` automatically calculates this to prevent socket framing desynchronization.
- **Heartbeat Frames**: Ping packets `~h~<num>` are echoed directly as `~m~<len>~m~~h~<num>`.
