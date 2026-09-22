# TradingView Python Bindings (`tradingview`)

High-performance Python bindings for the `tradingview-rs` asynchronous TradingView data provider, implemented in Rust via PyO3 0.29 and Maturin.

Features institutional-grade historical OHLCV data retrieval, real-time quote and candlestick streaming via WebSocket, corporate fundamental financial metrics, and global economic calendar events with direct [Polars](https://pola.rs) DataFrame integration.

---

## Features

- **Direct Polars Integration**: Fetch historical candles, batch series, fundamentals, and economic calendar events directly as Polars DataFrames (`as_dataframe=True`).
- **Zero GIL Contention**: Long-running network requests, batch iterations, and deserialization execute asynchronously on a background Tokio runtime while releasing the Python Global Interpreter Lock (GIL).
- **Dual-Mode Streaming**: Subscribe to live quotes and in-flight candlesticks using native async iterators (`async for`) or synchronous callbacks (`add_callback`) dispatched on the asyncio event loop with exception isolation (`sys.unraisablehook`).
- **Strict Protocol Parity**: Inherits `tradingview-rs`'s exact UTF-16 code-unit packet framing, 1:1 heartbeat echoing, and session management.
- **Typed & Tested**: 100% type annotated with `.pyi` type stubs, PEP 561 `py.typed` marker, and comprehensive automated test suite.

---

## Installation

```bash
pip install tradingview
```

To enable Polars and Pandas support:

```bash
pip install "tradingview[polars,pandas]"
```

---

## Quick Start

### 1. Historical Candlesticks Directly to Polars

```python
import asyncio
from tradingview import TradingViewClient, Interval

async def main():
    client = TradingViewClient()

    # Fetch 100 daily bars directly as a Polars DataFrame
    df = await client.get_historical("AAPL", "NASDAQ", Interval.OneDay, n_bars=100, as_dataframe=True)
    print(df)

    # Or retrieve structured HistoricalSeries with .to_polars() and .to_pandas()
    series = await client.get_historical("BTCUSDT", "BINANCE", Interval.OneHour, n_bars=50)
    polars_df = series.to_polars()
    latest = series[-1]
    print(f"Latest Bar: Close={latest.close}, Vol={latest.volume}")

    # Concurrent batch retrieval as a dictionary of DataFrames
    batch = await client.get_historical_batch(
        [("AAPL", "NASDAQ"), ("MSFT", "NASDAQ")],
        interval=Interval.OneDay,
        n_bars=30,
        as_dataframe=True,
    )
    print("AAPL rows:", batch["NASDAQ:AAPL"].height)

    await client.close()

asyncio.run(main())
```

### 2. Real-Time Quotes & Candlestick Streaming

```python
import asyncio
from tradingview import TradingViewClient, Interval, QuoteTick, CandleUpdate

def on_quote(tick: QuoteTick):
    print(f"[Callback] {tick.symbol} Price={tick.price} Bid={tick.bid} Ask={tick.ask}")

def on_candle(candle: CandleUpdate):
    print(f"[Callback] {candle.symbol} Close={candle.close} High={candle.high} Low={candle.low}")

async def main():
    client = TradingViewClient()

    # 1. Quote streaming with callback & async iterator
    quote_sub = await client.subscribe_quotes(["BINANCE:BTCUSDT"], callback=on_quote)

    async for tick in quote_sub:
        print(f"[Iterator] Tick: {tick.symbol} @ {tick.price}")
        break
    await quote_sub.stop()

    # 2. Live in-flight 1-minute candle streaming
    candle_sub = await client.subscribe_bars(["BINANCE:ETHUSDT"], interval=Interval.OneMinute, callback=on_candle)

    async for candle in candle_sub:
        print(f"[Iterator] Live Candle: {candle.symbol} Close={candle.close} Vol={candle.volume}")
        break
    await candle_sub.stop()

    await client.close()

asyncio.run(main())
```

### 3. Corporate Fundamentals & Economic Calendar

```python
import asyncio
from tradingview import TradingViewClient, FinancialPeriod, EconomicImportance

async def main():
    client = TradingViewClient()

    # Query corporate revenue history as a Polars DataFrame
    fund_df = await client.get_fundamental(
        "AAPL", "NASDAQ", "total_revenue", FinancialPeriod.FiscalYear, n_bars=5, as_dataframe=True
    )
    print(fund_df)

    # Query macroeconomic releases
    events_df = await client.get_economic_calendar(
        countries=["US"], min_importance=EconomicImportance.High, as_dataframe=True
    )
    print(events_df.select(["date", "country", "title", "indicator", "actual", "forecast"]))

    await client.close()

asyncio.run(main())
```

---

## License

MIT License.
