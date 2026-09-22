"""Pytest configuration and shared fixtures for tradingview-py test suite."""

import os

import pytest


@pytest.fixture
def auth_token() -> str:
    """Return auth token from environment or default anonymous token."""
    return os.getenv("TRADINGVIEW_AUTH_TOKEN", "unauthorized_user_token")


@pytest.fixture
def test_symbols() -> list[str]:
    """Return standard test symbols."""
    return ["BINANCE:BTCUSDT", "NASDAQ:AAPL"]
