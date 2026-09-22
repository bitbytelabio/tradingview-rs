"""Unit and integration tests for client authentication and lifecycle."""

import pytest
from tradingview import TradingViewClient
from tradingview.exceptions import AuthenticationError

def test_client_init_with_token() -> None:
    client = TradingViewClient(auth_token="test_token_123")
    assert client.auth_token == "test_token_123"
    assert client.is_authenticated is True
    assert client.username is None


def test_client_init_anonymous() -> None:
    client = TradingViewClient()
    assert client.auth_token is None
    assert client.is_authenticated is False


@pytest.mark.asyncio
async def test_client_login_invalid_credentials() -> None:
    with pytest.raises(AuthenticationError):
        await TradingViewClient.login(
            username="invalid_test_user_xyz",
            password="wrong_password_123",
        )


@pytest.mark.asyncio
async def test_client_authenticate_instance() -> None:
    client = TradingViewClient()
    assert client.is_authenticated is False
    with pytest.raises(AuthenticationError):
        await client.authenticate(
            username="invalid_test_user_xyz",
            password="wrong_password_123",
        )
    await client.close()
