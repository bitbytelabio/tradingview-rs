# Python API Reference (`tradingview`)

The `tradingview` Python library provides native, high-performance bindings to `tradingview-rs` compiled with PyO3 0.29 and Maturin.

---

## Architecture & Threading Model

- **Tokio-to-AsyncIO Bridge**: Operations execute asynchronously on a background Tokio runtime via `pyo3-async-runtimes`. All client methods return standard Python `asyncio` awaitables/coroutines.
- **Global Interpreter Lock (GIL) Release**: All network I/O, batch tasks, and JSON/WebSocket deserialization release the Python GIL, allowing true multicore parallelism.
- **HTTP & WebSocket Transports**: Authenticated user flows (`login`, 2FA TOTP verification, and session token retrieval via `get_tradingview_token`) use `wreq` and `wreq-util` with browser emulation and BoringSSL under the `user` feature. All public and unauthenticated HTTP REST requests (e.g. quote search, chart tokens, economic calendar, Pine catalog) use `reqwest`. WebSocket streaming connects via Tokio tungstenite; the Cargo `rustls-tls` feature applies to `reqwest` and WebSocket transport.
- **Native Toolchain Requirement**: Building the native library from source requires CMake, Clang (or GCC), and Perl to compile BoringSSL. Pre-built wheels are not guaranteed across all environments.
- **Callback Dispatching & Trampoline**: Callbacks registered on subscriptions are invoked on the Python event loop thread via `loop.call_soon_threadsafe` with exception isolation (`sys.unraisablehook`). Unhandled callback exceptions do not crash the stream.

---

## Client: `TradingViewClient`

```python
from tradingview import (
    TradingViewClient,
    DataServer,
    Interval,
    FinancialPeriod,
    EconomicImportance,
)
```

### Initializing Client

```python
# Anonymous client (default server is DataServer.Data, uses default 'unauthorized_user_token')
client = TradingViewClient()

# Client initialized with a pre-authenticated session token and default Data server
client = TradingViewClient(auth_token="your_session_auth_token")

# Client configured for TradingView ProData endpoint
client = TradingViewClient(
    auth_token="your_session_auth_token",
    server=DataServer.ProData,
)

# Check authentication state and configured server
print(client.is_authenticated)  # True or False
print(client.auth_token)        # Token string or None
print(client.server)            # DataServer.Data or DataServer.ProData
```

### DataServer Endpoints & Entitlements Notice

The `DataServer` enum provides endpoints for TradingView WebSocket communication:
- `DataServer.Data` (`"data"`): Default standard data server.
- `DataServer.ProData` (`"prodata"`): High-throughput data server for authenticated Pro accounts.
- `DataServer.WidgetData` (`"widgetdata"`): Widget data server.
- `DataServer.MobileData` (`"mobile-data"`): Mobile application data server.

> **Important Account Entitlements Notice**:
> Anonymous connection to `DataServer.ProData` is supported for public market data. However, accessing paid market data feeds requires corresponding account and feed entitlements; changing the endpoint to `ProData` does **not** grant paid data access or bypass feed entitlement checks without appropriate account subscriptions.
> **Application Environment Variable Responsibility**:
> Loading authentication tokens or login credentials from `.env` files or system environment variables is an explicit application concern (e.g. using `python-dotenv` or `os.getenv`). The library never automatically loads `.env` files.

### Credential Login & Authenticate

```python
# Asynchronous classmethod login: returns authenticated client
# totp_secret supports either standard RFC 6238 Base32 or a full otpauth:// URI (e.g. from Bitwarden)
# captcha_key is an optional 2Captcha API key to automatically solve reCAPTCHA v2 challenges
client = await TradingViewClient.login(
    username="your_username",
    password="your_password",
    totp_secret="OPTIONAL_2FA_TOTP_SECRET_OR_OTPAUTH_URI",
    captcha_key="OPTIONAL_2CAPTCHA_API_KEY",
    server=DataServer.ProData,  # Optional keyword argument (default: DataServer.Data)
)

# Or authenticate an existing client instance
await client.authenticate(
    username="your_username",
    password="your_password",
    totp_secret="OPTIONAL_2FA_TOTP_SECRET_OR_OTPAUTH_URI",
    captcha_key="OPTIONAL_2CAPTCHA_API_KEY",
```

### Automated CAPTCHA Solving (2Captcha Integration)

When TradingView requires a CAPTCHA verification challenge during automated sign-in (`recaptcha_required`), the Python client can automatically solve it using [2Captcha](https://2captcha.com) by supplying `captcha_key`:

```python
client = await TradingViewClient.login(
    username=os.environ["TV_USERNAME"],
    password=os.environ["TV_PASSWORD"],
    totp_secret=os.getenv("TV_TOTP_SECRET"),
    captcha_key=os.getenv("TWO_CAPTCHA_API_KEY"),
)
```

#### CAPTCHA Solving Details & Safeguards
- **Target reCAPTCHA Type**: TradingView uses standard **reCAPTCHA v2** (visible checkbox, `RecaptchaV2TaskProxyless` on sitekey `6Lcqv24UAAAAAIvkElDvwPxD0R8scDnMpizaBcHQ` and domain `recaptcha.net`), sending the solved token in form field `g-recaptcha-response-v2`. (Note: reCAPTCHA v3 is not used for email/password sign-in).
- **Strict Spending Control**: The solver is **never** invoked unconditionally. The client first sends regular credentials; only when TradingView responds with `recaptcha_required` does it dispatch at most **one** paid `createTask` task.
- **Never Called on Non-CAPTCHA Failures**: The solver is skipped on invalid credentials, 2FA errors, HTTP 429 or HTTP 200 `rate_limit` responses, and server errors, preventing wasted balance.
- **Timeouts & Deadlines**: Overall solve polling is bounded to a 120-second deadline (5-second polling interval) with 30-second HTTP request timeouts.
- **Automated Refund Reporting (`reportIncorrect`)**: If TradingView rejects the solved token on retry, the client automatically submits a single feedback complaint via [2Captcha reportIncorrect](https://2captcha.com/api-docs/report-incorrect). Complaints are reviewed by 2Captcha and refunds are subject to provider review (not guaranteed). Unsolvable tasks (`ERROR_CAPTCHA_UNSOLVABLE`, error 12) are automatically refunded by 2Captcha.

### Session & TradingView Token Retrieval (`get_tradingview_token`)

```python
async def get_tradingview_token(self) -> str: ...
```

Retrieves a TradingView session token using authenticated session cookies (`sessionid`, `sessionid_sign`, `device_t`).

- **Prerequisites**: The client must have established an authenticated cookie session via `TradingViewClient.login()` or `client.authenticate()`.
- **Rejection**: Anonymous clients or token-only clients initialized via `TradingViewClient(auth_token=...)` will immediately raise `AuthenticationError` before executing any network request. A supplied auth/websocket token is not a cookie session.
- **Token Disparity & Transport**: Token types are not equivalent; layout-sharing JWTs or standalone auth tokens do not grant cookie-authenticated HTTP session access. Authenticated cookie flows use `wreq` backed by BoringSSL, while public HTTP requests use `reqwest`; the Cargo `rustls-tls` feature applies to `reqwest` and WebSocket transport. No claim of CAPTCHA bypass is made.
- **Token Usage**: Returns the retrieved token string without mutating the current client's `auth_token`. The token can then be passed to instantiate a client targeting `DataServer.ProData`.

#### Example: Login, Retrieve Token, and Connect to ProData

```python
import os
from dotenv import load_dotenv
from tradingview import TradingViewClient, DataServer

load_dotenv()

# 1. Login with credentials to establish authenticated session cookies
login_client = await TradingViewClient.login(
    username=os.environ["TV_USERNAME"],
    password=os.environ["TV_PASSWORD"],
)

# 2. Retrieve the TradingView session token using session cookies
token = await login_client.get_tradingview_token()
await login_client.close()

# 3. Instantiate a data client using the retrieved token and ProData endpoint
data_client = TradingViewClient(
    auth_token=token,
    server=DataServer.ProData,
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
    *,
    server: DataServer | None = None,
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
    *,
    server: DataServer | None = None,
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
    *,
    server: DataServer | None = None,
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
    *,
    server: DataServer | None = None,
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
    *,
    server: DataServer | None = None,
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
