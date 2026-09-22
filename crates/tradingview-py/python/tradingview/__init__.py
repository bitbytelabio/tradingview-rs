"""TradingView Data Provider Python API backed by tradingview-rs."""

try:
    from tradingview._tradingview import (  # type: ignore[import-not-found]
        # Exceptions
        TradingViewError,
        AuthenticationError,
        SymbolNotFoundError,
        ConnectionError,
        TimeoutError,
        RateLimitError,
        ProtocolError,
        # Enums
        Interval,
        FinancialPeriod,
        EconomicImportance,
        # Models
        Bar,
        CandleUpdate,
        HistoricalSeries,
        QuoteTick,
        FundamentalPoint,
        FundamentalSeries,
        EconomicEvent,
        # Subscriptions
        QuoteSubscription,
        BarSubscription,
        # Client
        TradingViewClient,
    )
except ImportError:
    # Allow importing package during build or before native extension is compiled
    pass

__all__ = [
    "TradingViewError",
    "AuthenticationError",
    "SymbolNotFoundError",
    "ConnectionError",
    "TimeoutError",
    "RateLimitError",
    "ProtocolError",
    "Interval",
    "FinancialPeriod",
    "EconomicImportance",
    "Bar",
    "CandleUpdate",
    "HistoricalSeries",
    "QuoteTick",
    "FundamentalPoint",
    "FundamentalSeries",
    "EconomicEvent",
    "QuoteSubscription",
    "BarSubscription",
    "TradingViewClient",
]
