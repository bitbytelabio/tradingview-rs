"""Domain-specific exception hierarchy for TradingView Python API."""

from tradingview import (
    TradingViewError,
    AuthenticationError,
    SymbolNotFoundError,
    ConnectionError,
    TimeoutError,
    RateLimitError,
    ProtocolError,
)

__all__ = [
    "TradingViewError",
    "AuthenticationError",
    "SymbolNotFoundError",
    "ConnectionError",
    "TimeoutError",
    "RateLimitError",
    "ProtocolError",
]
