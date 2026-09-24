# Quickstart & Verification Guide

This guide walks through getting started with the TradingView Data Provider in both **Python** and **Rust**, covering historical data retrieval, direct Polars conversion, real-time streaming, and corporate fundamental queries.

---

## 1. Python Quickstart

### Prerequisites & Setup

```bash
# Recommended: Create a clean virtual environment using uv or venv
uv venv --python 3.12 .venv
source .venv/bin/activate

# Install the tradingview package with Polars and Pandas support
# Note: Building from source requires CMake, Clang (or GCC), and Perl to compile wreq / BoringSSL.
pip install "tradingview-rs[polars,pandas]"
```

### Scenario 1: Historical OHLCV Directly to Polars

```python
import asyncio
from tradingview import TradingViewClient, Interval

async def main():
    client = TradingViewClient()

    # 1. Fetch historical OHLCV data directly as a Polars DataFrame
    df = await client.get_historical(
        symbol="AAPL",
        exchange="NASDAQ",
        interval=Interval.OneDay,
        n_bars=100,
        as_dataframe=True,
    )
    print("Historical DataFrame (Polars):")
    print(df)

    # 2. Or retrieve structured HistoricalSeries with .to_polars() and .to_pandas()
    series = await client.get_historical("BTCUSDT", "BINANCE", Interval.OneHour, n_bars=50)
    print(f"Retrieved {len(series)} bars for {series.symbol} on {series.exchange}")
    first_bar = series[0]
    print(f"First Bar: O={first_bar.open}, H={first_bar.high}, L={first_bar.low}, C={first_bar.close}, V={first_bar.volume}")

    # 3. Concurrent batch retrieval as a dictionary of DataFrames
    batch_df = await client.get_historical_batch(
        symbols=[("AAPL", "NASDAQ"), ("MSFT", "NASDAQ")],
        interval=Interval.OneDay,
        n_bars=30,
        as_dataframe=True,
    )
    print("AAPL rows:", batch_df["NASDAQ:AAPL"].height)
    print("MSFT rows:", batch_df["NASDAQ:MSFT"].height)

    await client.close()

asyncio.run(main())
```

### Scenario 1b: Using the ProData Server Endpoint with TradingView Token

```python
import asyncio
import os
from dotenv import load_dotenv
from tradingview import TradingViewClient, DataServer, Interval

# Entitlements & Environment Notice:
# Anonymous connection to DataServer.ProData is supported for public market data.
# However, accessing paid market data feeds requires account and feed entitlements;
# merely switching the endpoint to ProData does not grant paid access or bypass paywalled feeds.
# Loading credentials or tokens from .env is an explicit application concern (e.g. using python-dotenv).
# An explicit login session retrieves a TradingView token, which can then be passed to a ProData client.
# Note: Token types are not equivalent. A supplied TV_AUTH_TOKEN does not constitute a cookie session
# (token-only clients cannot call get_tradingview_token). Cookie authentication uses session cookies via wreq;
# no CAPTCHA bypass is claimed. totp_secret supports either standard RFC 6238 Base32 or full otpauth:// URI.
load_dotenv()

async def main():
    username = os.getenv("TV_USERNAME")
    password = os.getenv("TV_PASSWORD")
    if username and password:
        # 1. Login with credentials to establish authenticated session cookies
        login_client = await TradingViewClient.login(username=username, password=password)
        # 2. Retrieve TradingView session token using session cookies
        token = await login_client.get_tradingview_token()
        await login_client.close()
    else:
        # Fall back to pre-configured auth token if set
        token = os.getenv("TV_AUTH_TOKEN")

    # 3. Instantiate client with token and ProData endpoint
    client = TradingViewClient(auth_token=token, server=DataServer.ProData)

    df = await client.get_historical(
        symbol="AAPL",
        exchange="NASDAQ",
        interval=Interval.OneDay,
        n_bars=50,
        as_dataframe=True,
    )
    print(f"ProData DataFrame shape: {df.shape}")
    await client.close()

asyncio.run(main())
```

### Scenario 2: Real-Time Quotes & Candlestick Streaming

```python
import asyncio
from tradingview import TradingViewClient, Interval, QuoteTick, CandleUpdate

def on_quote(tick: QuoteTick):
    print(f"[Callback] {tick.symbol} Price={tick.price} Bid={tick.bid} Ask={tick.ask}")

def on_candle(candle: CandleUpdate):
    print(f"[Callback] {candle.symbol} [{candle.interval.value}] C={candle.close} H={candle.high} L={candle.low}")

async def main():
    client = TradingViewClient()

    # 1. Stream live quotes with both synchronous callback and async iterator
    quote_sub = await client.subscribe_quotes(["BINANCE:BTCUSDT"], callback=on_quote)

    count = 0
    print("Awaiting real-time price ticks...")
    async for tick in quote_sub:
        print(f"[Async Iterator] Tick: {tick.symbol} @ {tick.price} (Vol: {tick.volume})")
        count += 1
        if count >= 3:
            break
    await quote_sub.stop()

    # 2. Stream in-flight 1-minute candlestick updates
    candle_sub = await client.subscribe_bars(["BINANCE:ETHUSDT"], interval=Interval.OneMinute, callback=on_candle)

    count = 0
    print("Awaiting live in-flight candles...")
    async for candle in candle_sub:
        print(f"[Async Iterator] Candle: {candle.symbol} Close={candle.close} Volume={candle.volume}")
        count += 1
        if count >= 2:
            break
    await candle_sub.stop()

    await client.close()
    print("Streaming streams cleanly closed.")

asyncio.run(main())
```

### Scenario 3: Corporate Fundamentals & Global Economic Calendar

```python
import asyncio
from tradingview import TradingViewClient, FinancialPeriod, EconomicImportance

async def main():
    client = TradingViewClient()

    # 1. Query corporate revenue history directly as a Polars DataFrame
    fund_df = await client.get_fundamental(
        symbol="AAPL",
        exchange="NASDAQ",
        fund_id="total_revenue",
        period=FinancialPeriod.FiscalYear,
        n_bars=5,
        as_dataframe=True,
    )
    print("Apple Annual Total Revenue:")
    print(fund_df)

    # 2. Query upcoming high-importance macroeconomic releases for the US
    events_df = await client.get_economic_calendar(
        countries=["US"],
        min_importance=EconomicImportance.High,
        as_dataframe=True,
    )
    print("High-Importance US Economic Calendar Events:")
    print(events_df.select(["date", "country", "title", "indicator", "actual", "forecast"]))

    await client.close()

asyncio.run(main())
```

---

## 2. Rust Quickstart

### Prerequisites & Setup

Add `tradingview-rs` to your `Cargo.toml`:

```toml
[dependencies]
tradingview-rs = "0.3"
tokio = { version = "1", features = ["full"] }
```

### Scenario 1: Historical Candlestick Retrieval (Single & Batch)

```rust
use tradingview::historical::{BatchConfig, HistoricalClient, HistoricalRequest};
use tradingview::live::models::DataServer;
use tradingview::models::Interval;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = HistoricalClient::new("unauthorized_user_token", DataServer::Data);

    // 1. Single symbol historical fetch
    let req = HistoricalRequest::builder()
        .symbol("AAPL")
        .exchange("NASDAQ")
        .interval(Interval::OneDay)
        .num_bars(100)
        .build();

    let result = client.retrieve(req).await?;
    println!("Retrieved {} daily bars for {}", result.len(), result.symbol_info.name);
    if let Some(first_bar) = result.data.first() {
        println!("First Bar: TS={}, Close={}", first_bar.timestamp, first_bar.close);
    }

    // 2. Concurrent multi-symbol batch fetch
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
                ..Default::default()
            },
        )
        .await;

    println!("Batch retrieval completed: {} succeeded, {} failed", batch.successful.len(), batch.failed.len());

    Ok(())
}
```

### Scenario 2: Real-Time WebSocket Quote Streaming

```rust
use serde_json::Value;
use std::sync::Arc;
use tokio::signal;
use tradingview::live::{handler::Handler, models::TradingViewDataEvent, websocket::WebSocketClient};
use tradingview::{DataServer, Error};

struct QuotePrinter;

impl Handler for QuotePrinter {
    fn handle_events(&self, event: TradingViewDataEvent, message: &[Value]) {
        if event == TradingViewDataEvent::OnQuoteData {
            println!("Quote update received: {:?}", message);
        }
    }
    fn handle_quote_data(&self, message: &[Value]) {
        println!("Legacy quote update: {:?}", message);
    }
    fn handle_series_data(&self, _event: TradingViewDataEvent, _messages: &[Value]) {}
    fn notify_error(&self, error: Error, message: &[Value]) {
        eprintln!("Socket error: {:?}, payload: {:?}", error, message);
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ws = WebSocketClient::builder()
        .auth_token("unauthorized_user_token")
        .server(DataServer::Data)
        .handler(QuotePrinter)
        .build()
        .await?;

    Arc::clone(&ws).spawn_reader_task();

    let session = tradingview::utils::gen_session_id("qs");
    ws.create_quote_session(&session).await?;
    ws.set_fields(&session).await?;
    ws.add_symbols(&session, &["BINANCE:BTCUSDT"]).await?;

    println!("Streaming live quotes. Press Ctrl+C to stop.");
    signal::ctrl_c().await?;

    ws.delete_quote_session(&session).await?;
    ws.close().await?;
    Ok(())
}
```

### Scenario 3: Event-Driven `DataLoader` Pipeline

```rust
use std::sync::Arc;
use tradingview::loader::DataLoader;
use tradingview::sink::callback::CallbackSink;
use tradingview::sink::channel::ChannelSink;
use tradingview::source::tradingview::WebSocketSource;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Setup channel sink
    let (channel_sink, mut rx) = ChannelSink::new(1024);

    // 2. Setup callback sink
    let callback_sink = CallbackSink::new(|event| {
        println!("[Callback Sink] Event: {:?}", event);
        Ok(())
    });

    // 3. Build WebSocket data source
    let source = WebSocketSource::builder()
        .auth_token("unauthorized_user_token")
        .symbols(vec!["BINANCE:BTCUSDT".to_string()])
        .build()?;

    // 4. Assemble DataLoader pipeline
    let mut loader = DataLoader::builder()
        .source(Box::new(source))
        .add_sink(Arc::new(channel_sink))
        .add_sink(Arc::new(callback_sink))
        .build()?;

    let handle = loader.start().await?;

    // 5. Consume from channel
    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            println!("[Channel Sink] Received: {:?}", event);
        }
    });

    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    handle.stop().await?;
    println!("Pipeline stopped successfully.");

    Ok(())
}
```
