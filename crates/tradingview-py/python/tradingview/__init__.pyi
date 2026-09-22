"""TradingView Data Provider Python API bindings backed by tradingview-rs."""

from collections.abc import AsyncIterator, Callable, Iterator, Sequence
from enum import Enum
from typing import Any, TypeAlias

from tradingview.exceptions import (
    AuthenticationError,
    ConnectionError,
    ProtocolError,
    RateLimitError,
    SymbolNotFoundError,
    TimeoutError,
    TradingViewError,
)

class Interval(Enum):
    OneMinute = "1"
    ThreeMinutes = "3"
    FiveMinutes = "5"
    FifteenMinutes = "15"
    ThirtyMinutes = "30"
    FortyFiveMinutes = "45"
    OneHour = "60"
    TwoHours = "120"
    ThreeHours = "180"
    FourHours = "240"
    OneDay = "1D"
    OneWeek = "1W"
    OneMonth = "1M"

class FinancialPeriod(Enum):
    FiscalYear = "FY"
    FiscalQuarter = "FQ"
    FiscalHalfYear = "FH"
    TrailingTwelveMonths = "TTM"
    NoAggregation = "NOAGG"

class EconomicImportance(Enum):
    Low = -1
    Medium = 0
    High = 1

class Bar:
    """Individual historical OHLCV price bar."""

    timestamp: int
    open: float
    high: float
    low: float
    close: float
    volume: float

    def __init__(
        self,
        timestamp: int,
        open: float,
        high: float,
        low: float,
        close: float,
        volume: float,
    ) -> None: ...
    def to_dict(self) -> dict[str, Any]: ...
    def to_tuple(self) -> tuple[int, float, float, float, float, float]: ...

class CandleUpdate:
    """Real-time in-flight candle progress or closed bar update."""

    symbol: str
    interval: Interval
    timestamp: int
    open: float
    high: float
    low: float
    close: float
    volume: float

    def __init__(
        self,
        symbol: str,
        interval: Interval,
        timestamp: int,
        open: float,
        high: float,
        low: float,
        close: float,
        volume: float,
    ) -> None: ...
    def to_dict(self) -> dict[str, Any]: ...
    def to_bar(self) -> Bar: ...

class HistoricalSeries:
    """Ordered collection of price bars for a symbol."""

    symbol: str
    exchange: str
    interval: Interval
    bars: list[Bar]

    def __init__(
        self,
        symbol: str,
        exchange: str,
        interval: Interval,
        bars: list[Bar],
    ) -> None: ...
    def __len__(self) -> int: ...
    def __getitem__(self, index: int) -> Bar: ...
    def __iter__(self) -> Iterator[Bar]: ...
    def to_dict(self) -> dict[str, Any]: ...
    def to_polars(self) -> Any: ...
    def to_pandas(self) -> Any: ...

class QuoteTick:
    """Real-time market quote update."""

    symbol: str
    timestamp: int
    price: float
    volume: float
    bid: float | None
    ask: float | None
    change: float | None
    change_percent: float | None

    def __init__(
        self,
        symbol: str,
        timestamp: int,
        price: float,
        volume: float,
        bid: float | None = None,
        ask: float | None = None,
        change: float | None = None,
        change_percent: float | None = None,
    ) -> None: ...
    def to_dict(self) -> dict[str, Any]: ...

class FundamentalPoint:
    """Individual fundamental indicator point."""

    timestamp: int
    value: float
    index: int

    def __init__(
        self,
        timestamp: int,
        value: float,
        index: int,
    ) -> None: ...
    def to_dict(self) -> dict[str, Any]: ...

class FundamentalSeries:
    """Time series of fundamental data points."""

    symbol: str
    exchange: str
    fund_id: str
    period: FinancialPeriod
    points: list[FundamentalPoint]

    def __init__(
        self,
        symbol: str,
        exchange: str,
        fund_id: str,
        period: FinancialPeriod,
        points: list[FundamentalPoint],
    ) -> None: ...
    def __len__(self) -> int: ...
    def __getitem__(self, index: int) -> FundamentalPoint: ...
    def __iter__(self) -> Iterator[FundamentalPoint]: ...
    def to_dict(self) -> dict[str, Any]: ...
    def to_polars(self) -> Any: ...
    def to_pandas(self) -> Any: ...

class EconomicEvent:
    """Scheduled global macroeconomic event."""

    id: str
    title: str
    country: str
    indicator: str
    ticker: str
    date: int
    importance: EconomicImportance
    actual: float | None
    forecast: float | None
    previous: float | None

    def __init__(
        self,
        id: str,
        title: str,
        country: str,
        indicator: str,
        ticker: str,
        date: int,
        importance: EconomicImportance,
        actual: float | None = None,
        forecast: float | None = None,
        previous: float | None = None,
    ) -> None: ...
    def to_dict(self) -> dict[str, Any]: ...

QuoteCallback: TypeAlias = Callable[[QuoteTick], None]
CandleCallback: TypeAlias = Callable[[CandleUpdate], None]

class QuoteSubscription:
    """Active real-time market quote stream subscription."""
    def __aiter__(self) -> AsyncIterator[QuoteTick]: ...
    async def __anext__(self) -> QuoteTick: ...
    def add_callback(self, callback: QuoteCallback) -> None: ...
    async def stop(self) -> None: ...

class BarSubscription:
    """Active real-time candlestick bar progress subscription."""
    def __aiter__(self) -> AsyncIterator[CandleUpdate]: ...
    async def __anext__(self) -> CandleUpdate: ...
    def add_callback(self, callback: CandleCallback) -> None: ...
    async def stop(self) -> None: ...

class TradingViewClient:
    """Main client interface for accessing TradingView market data."""

    auth_token: str | None
    username: str | None
    is_authenticated: bool

    def __init__(self, auth_token: str | None = None) -> None: ...
    @classmethod
    async def login(
        cls,
        username: str,
        password: str,
        totp_secret: str | None = None,
    ) -> TradingViewClient: ...
    async def authenticate(
        self,
        username: str,
        password: str,
        totp_secret: str | None = None,
    ) -> None: ...
    async def get_historical(
        self,
        symbol: str,
        exchange: str,
        interval: Interval = Interval.OneDay,
        n_bars: int = 100,
        with_replay: bool = False,
        as_dataframe: bool = False,
    ) -> Any: ...
    async def get_historical_df(
        self,
        symbol: str,
        exchange: str,
        interval: Interval = Interval.OneDay,
        n_bars: int = 100,
        with_replay: bool = False,
    ) -> Any: ...
    async def get_historical_batch(
        self,
        symbols: Sequence[tuple[str, str]],
        interval: Interval = Interval.OneDay,
        n_bars: int = 100,
        max_concurrency: int = 4,
        as_dataframe: bool = False,
    ) -> Any: ...
    async def subscribe_quotes(
        self,
        symbols: Sequence[str],
        callback: QuoteCallback | None = None,
    ) -> QuoteSubscription: ...
    async def subscribe_bars(
        self,
        symbols: Sequence[str],
        interval: Interval = Interval.OneMinute,
        callback: CandleCallback | None = None,
    ) -> BarSubscription: ...
    async def get_fundamental(
        self,
        symbol: str,
        exchange: str,
        fund_id: str,
        period: FinancialPeriod = FinancialPeriod.FiscalYear,
        n_bars: int = 20,
        as_dataframe: bool = False,
    ) -> Any: ...
    async def get_economic_calendar(
        self,
        countries: Sequence[str] | None = None,
        from_timestamp: int | None = None,
        to_timestamp: int | None = None,
        min_importance: EconomicImportance = EconomicImportance.Medium,
        as_dataframe: bool = False,
    ) -> Any: ...
    async def close(self) -> None: ...

__all__ = [
    "AuthenticationError",
    "Bar",
    "BarSubscription",
    "CandleUpdate",
    "ConnectionError",
    "EconomicEvent",
    "EconomicImportance",
    "FinancialPeriod",
    "FundamentalPoint",
    "FundamentalSeries",
    "HistoricalSeries",
    "Interval",
    "ProtocolError",
    "QuoteSubscription",
    "QuoteTick",
    "RateLimitError",
    "SymbolNotFoundError",
    "TimeoutError",
    "TradingViewClient",
    "TradingViewError",
]
