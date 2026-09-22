"""Pytest configuration and shared fixtures for tradingview-py test suite."""

import asyncio
import os
import pytest
from typing import AsyncGenerator, Generator

@pytest.fixture(scope="session")
def event_loop() -> Generator[asyncio.AbstractEventLoop, None, None]:
    """Create a session-scoped asyncio event loop."""
    loop = asyncio.new_event_loop()
    asyncio.set_event_loop(loop)
    yield loop
    loop.close()

@pytest.fixture
def auth_token() -> str:
    """Return auth token from environment or default anonymous token."""
    return os.getenv("TRADINGVIEW_AUTH_TOKEN", "unauthorized_user_token")

@pytest.fixture
def test_symbols() -> list[str]:
    """Return standard test symbols."""
    return ["BINANCE:BTCUSDT", "NASDAQ:AAPL"]
