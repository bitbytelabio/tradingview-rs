"""TradingView Data Provider Python API backed by tradingview-rs."""

# Import native extension
try:
    from tradingview import _tradingview as _tv
except ImportError:
    try:
        import _tradingview as _tv  # type: ignore[no-redef]
    except ImportError:
        _tv = None  # type: ignore[assignment]

if _tv is not None:
    _symbols = [
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
        "DataServer",
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
    for _sym in _symbols:
        if hasattr(_tv, _sym):
            globals()[_sym] = getattr(_tv, _sym)

__all__ = [
    "AuthenticationError",
    "Bar",
    "BarSubscription",
    "CandleUpdate",
    "ConnectionError",
    "DataServer",
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
