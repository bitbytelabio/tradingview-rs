"""Domain-specific exception hierarchy for TradingView Python API."""

class TradingViewError(Exception):
    """Base exception for all TradingView client errors."""
    pass

class AuthenticationError(TradingViewError):
    """Raised when authentication credentials or token are invalid, expired, or rejected."""
    pass

class SymbolNotFoundError(TradingViewError):
    """Raised when a requested ticker symbol or exchange cannot be resolved or found."""
    pass

class ConnectionError(TradingViewError):
    """Raised when network transport, I/O, or WebSocket connection fails unexpectedly."""
    pass

class TimeoutError(TradingViewError):
    """Raised when a request or connection attempt exceeds the deadline."""
    pass

class RateLimitError(TradingViewError):
    """Raised when upstream TradingView rate limits or throttling are encountered."""
    pass

class ProtocolError(TradingViewError):
    """Raised when unexpected or unparseable frames are received from upstream."""
    pass

__all__ = [
    "TradingViewError",
    "AuthenticationError",
    "SymbolNotFoundError",
    "ConnectionError",
    "TimeoutError",
    "RateLimitError",
    "ProtocolError",
]
