# Python API Reference (`tradingview`)

The `tradingview` Python library provides native, high-performance bindings to `tradingview-rs` compiled with PyO3 0.29 and Maturin.

---

## Architecture & Threading Model

- **Tokio-to-AsyncIO Bridge**: Operations execute asynchronously on a background Tokio runtime via `pyo3-async-runtimes`. All client methods return standard Python `asyncio` awaitables/coroutines.
- **Global Interpreter Lock (GIL) Release**: All network I/O, batch tasks, and JSON/WebSocket deserialization release the Python GIL, allowing true multicore parallelism.
- **Callback Dispatching & Trampoline**: Callbacks registered on subscriptions are invoked on the Python event loop thread via `loop.call_soon_threadsafe` with exception isolation (`sys.unraisablehook`). Unhandled callback exceptions do not crash the stream.

---

## Client: `TradingViewClient`

```python
from tradingview import TradingViewClient, Interval, FinancialPeriod, EconomicImportance
```

### Initializing Client

```python
# Anonymous client (uses default 'unauthorized_user_token')
client = TradingViewClient()

# Client initialized with a pre-authenticated session token
client = TradingViewClient(auth_token="your_session_auth_token")

# Check authentication state
print(client.is_authenticated)  # True or False
print(client.auth_token)        # Token string or None
```

### Credential Login & Authenticate

```python
# Asynchronous classmethod login: returns authenticated client
client = await TradingViewClient.login(
    username="your_username",
    password="your_password",
    totp_secret="OPTIONAL_2FA_TOTP_SECRET",
)

# Or authenticate an existing client instance
await client.authenticate(
    username="your_username",
    password="your_password",
    totp_secret="OPTIONAL_2FA_TOTP_SECRET",
)
```

---

## Historical Data Retrieval

### `get_historical`

```python
async def get_historical(
    symbol: str,
    exchange: str,
    interval: Interval = Interval.OneDay,
    n_bars: int = 100,
    with_replay: bool = False,
    as_dataframe: bool = False,
) -> HistoricalSeries | polars.DataFrame: ...
```

- **`as_dataframe=True`**: Directly returns a `polars.DataFrame` with columns `["timestamp", "open", "high", "low", "close", "volume"]`.
- **`as_dataframe=False`**: Returns a `HistoricalSeries` container.

```python
# Direct Polars DataFrame
df = await client.get_historical("AAPL", "NASDAQ", Interval.OneDay, n_bars=100, as_dataframe=True)

# Convenience method
df = await client.get_historical_df("AAPL", "NASDAQ", Interval.OneDay, n_bars=100)

# Structured HistoricalSeries
series = await client.get_historical("AAPL", "NASDAQ", Interval.OneDay, n_bars=100)
print(len(series))          # Number of bars
first = series[0]           # First Bar
last = series[-1]           # Negative indexing supported
df_pl = series.to_polars()  # Convert to Polars DataFrame
df_pd = series.to_pandas()  # Convert to Pandas DataFrame
```

### `get_historical_batch`

```python
async def get_historical_batch(
    symbols: Sequence[tuple[str, str]],
    interval: Interval = Interval.OneDay,
    n_bars: int = 100,
    max_concurrency: int = 4,
    as_dataframe: bool = False,
) -> dict[str, HistoricalSeries] | dict[str, polars.DataFrame]: ...
```

- Retrieves bars concurrently for multiple instruments with a bounded concurrency semaphore.
- Results are keyed by `"EXCHANGE:SYMBOL"` (e.g. `"NASDAQ:AAPL"`, `"NASDAQ:MSFT"`).
- When `as_dataframe=True`, values are `polars.DataFrame` instances.

---

## Real-Time Streaming Subscriptions

### `subscribe_quotes`

```python
async def subscribe_quotes(
    symbols: Sequence[str],
    callback: Callable[[QuoteTick], None] | None = None,
) -> QuoteSubscription: ...
```

- **Lossy Queue Policy**: Drops older unconsumed snapshots if the internal 1024-item buffer fills, ensuring callers always receive the freshest price.
- **Dual Mode**: Yields `QuoteTick` instances via `async for` and/or synchronous callbacks.

```python
def on_tick(tick: QuoteTick):
    print(f"Callback: {tick.symbol} @ {tick.price} (bid={tick.bid}, ask={tick.ask})")

sub = await client.subscribe_quotes(["BINANCE:BTCUSDT", "NASDAQ:AAPL"], callback=on_tick)

# Asynchronous iterator
async for tick in sub:
    print(f"Iterator tick: {tick.symbol} price={tick.price}")
    break

# Cleanup
await sub.stop()
```

### `subscribe_bars`

```python
async def subscribe_bars(
    symbols: Sequence[str],
    interval: Interval = Interval.OneMinute,
    callback: Callable[[CandleUpdate], None] | None = None,
) -> BarSubscription: ...
```

- **Lossless Backpressured Queue Policy**: Awaits capacity on the internal buffer, guaranteeing no dropped candlestick closing events.

```python
sub = await client.subscribe_bars(["BINANCE:ETHUSDT"], interval=Interval.OneMinute)

async for candle in sub:
    print(f"Candle: {candle.symbol} Close={candle.close} High={candle.high}")
    break

await sub.stop()
```

---

## Fundamentals & Economic Calendar

### `get_fundamental`

```python
async def get_fundamental(
    symbol: str,
    exchange: str,
    fund_id: str,
    period: FinancialPeriod = FinancialPeriod.FiscalYear,
    n_bars: int = 20,
    as_dataframe: bool = False,
) -> FundamentalSeries | polars.DataFrame: ...
```

- `fund_id`: Metric identifier (e.g. `"total_revenue"`, `"net_income"`, `"ebitda"`, `"total_assets"`).
- `period`: `FinancialPeriod.FiscalYear` (`"FY"`), `FiscalQuarter` (`"FQ"`), `FiscalHalfYear` (`"FH"`), `TrailingTwelveMonths` (`"TTM"`), or `NoAggregation` (`"NOAGG"`).

### `get_economic_calendar`

```python
async def get_economic_calendar(
    countries: Sequence[str] | None = None,
    from_timestamp: int | None = None,
    to_timestamp: int | None = None,
    min_importance: EconomicImportance = EconomicImportance.Medium,
    as_dataframe: bool = False,
) -> list[EconomicEvent] | polars.DataFrame: ...
```

- `countries`: Optional list of 2-letter uppercase ISO country codes (`["US", "DE", "JP"]`).
- `min_importance`: `EconomicImportance.Low` (`-1`), `EconomicImportance.Medium` (`0`), `EconomicImportance.High` (`1`).

---

## Domain Exceptions (`tradingview.exceptions`)

```python
from tradingview.exceptions import (
    TradingViewError,
    AuthenticationError,
    SymbolNotFoundError,
    ConnectionError,
    TimeoutError,
    RateLimitError,
    ProtocolError,
)
```

| Exception | Base | When Raised |
| :--- | :--- | :--- |
| `TradingViewError` | `Exception` | Base class for all TradingView provider exceptions |
| `AuthenticationError` | `TradingViewError` | Invalid credentials, expired session, or rejected token |
| `SymbolNotFoundError` | `TradingViewError` | Symbol or exchange could not be resolved |
| `ConnectionError` | `TradingViewError` | WebSocket drop, HTTP connection failure, or I/O error |
| `TimeoutError` | `TradingViewError` | Request deadline exceeded |
| `RateLimitError` | `TradingViewError` | HTTP 429 or WebSocket throttling status received |
| `ProtocolError` | `TradingViewError` | Unexpected or unparseable frames received from server |
