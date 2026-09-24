"""Unit and integration tests for client authentication and lifecycle."""

from collections.abc import Callable

import pytest
from tradingview import (
    DataServer,
    FinancialPeriod,
    Interval,
    TradingViewClient,
)
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


@pytest.mark.asyncio
async def test_get_tradingview_token_anonymous_rejected() -> None:
    client = TradingViewClient()
    with pytest.raises(AuthenticationError, match="session cookies"):
        await client.get_tradingview_token()
    await client.close()


@pytest.mark.asyncio
async def test_get_tradingview_token_token_only_rejected() -> None:
    client = TradingViewClient(auth_token="jwt_or_websocket_token")
    assert client.is_authenticated is True
    with pytest.raises(
        AuthenticationError, match="supplied auth_token is not a cookie session"
    ):
        await client.get_tradingview_token()
    await client.close()


def test_get_tradingview_token_rejects_arguments() -> None:
    client = TradingViewClient()
    get_token: Callable[..., object] = client.get_tradingview_token
    with pytest.raises(TypeError):
        get_token("unexpected_argument")


def test_data_server_enum_properties() -> None:
    assert DataServer.Data.value == "data"
    assert DataServer.Data.name == "Data"
    assert DataServer.ProData.value == "prodata"
    assert DataServer.ProData.name == "ProData"
    assert DataServer.WidgetData.value == "widgetdata"
    assert DataServer.WidgetData.name == "WidgetData"
    assert DataServer.MobileData.value == "mobile-data"
    assert DataServer.MobileData.name == "MobileData"


def test_client_init_server_configuration() -> None:
    # Default server is consistently Data
    client_default = TradingViewClient()
    assert client_default.server == DataServer.Data

    # Explicit server configuration via keyword-only argument
    client_pro = TradingViewClient(server=DataServer.ProData)
    assert client_pro.server == DataServer.ProData

    client_widget = TradingViewClient(
        auth_token="my_token", server=DataServer.WidgetData
    )
    assert client_widget.server == DataServer.WidgetData
    assert client_widget.auth_token == "my_token"

    client_mobile = TradingViewClient(server=DataServer.MobileData)
    assert client_mobile.server == DataServer.MobileData


def test_client_server_immutability() -> None:
    client = TradingViewClient(server=DataServer.ProData)
    assert client.server == DataServer.ProData
    with pytest.raises(AttributeError):
        client.__setattr__("server", DataServer.Data)


def test_client_init_positional_server_rejected() -> None:
    # server must be keyword-only, cannot be passed as 2nd positional argument
    client_cls: Callable[..., object] = TradingViewClient
    with pytest.raises(TypeError):
        client_cls("token_123", DataServer.ProData)


def test_client_init_invalid_server_type() -> None:
    client_cls: Callable[..., object] = TradingViewClient
    with pytest.raises(TypeError):
        client_cls(server="prodata")


@pytest.mark.asyncio
async def test_client_authenticate_retains_server() -> None:
    client = TradingViewClient(server=DataServer.ProData)
    assert client.server == DataServer.ProData
    with pytest.raises(AuthenticationError):
        await client.authenticate(
            username="invalid_test_user_xyz",
            password="wrong_password_123",
        )
    # Server remains ProData even after failed authentication
    assert client.server == DataServer.ProData
    await client.close()


@pytest.mark.asyncio
async def test_client_login_server_keyword_only_and_type_check() -> None:
    # Positional server is rejected (login takes username, password, optional totp positionally)
    login_fn: Callable[..., object] = TradingViewClient.login
    with pytest.raises(TypeError):
        login_fn(
            "user",
            "pass",
            None,
            DataServer.ProData,
        )

    # Invalid type for keyword-only server is rejected
    with pytest.raises(TypeError):
        login_fn(
            username="user",
            password="pass",
            server="invalid_server",
        )


def test_method_server_keyword_only_and_type_boundaries() -> None:
    client = TradingViewClient(server=DataServer.Data)

    # get_historical: invalid server type
    get_historical: Callable[..., object] = client.get_historical
    with pytest.raises(TypeError):
        get_historical("AAPL", "NASDAQ", server="invalid")

    # get_historical: server passed positionally beyond signature
    with pytest.raises(TypeError):
        get_historical(
            "AAPL", "NASDAQ", Interval.OneDay, 100, False, False, DataServer.ProData
        )

    # get_historical_df: invalid server type
    get_historical_df: Callable[..., object] = client.get_historical_df
    with pytest.raises(TypeError):
        get_historical_df("AAPL", "NASDAQ", server=123)

    # get_historical_df: server passed positionally beyond signature
    with pytest.raises(TypeError):
        get_historical_df(
            "AAPL", "NASDAQ", Interval.OneDay, 100, False, DataServer.ProData
        )

    # get_historical_batch: invalid server type
    get_historical_batch: Callable[..., object] = client.get_historical_batch
    with pytest.raises(TypeError):
        get_historical_batch([("AAPL", "NASDAQ")], server="invalid")

    # get_historical_batch: server passed positionally beyond signature
    with pytest.raises(TypeError):
        get_historical_batch(
            [("AAPL", "NASDAQ")], Interval.OneDay, 100, 4, False, DataServer.ProData
        )

    # subscribe_quotes: invalid server type
    subscribe_quotes: Callable[..., object] = client.subscribe_quotes
    with pytest.raises(TypeError):
        subscribe_quotes(["BINANCE:BTCUSDT"], server="invalid")

    # subscribe_quotes: server passed positionally beyond signature
    with pytest.raises(TypeError):
        subscribe_quotes(["BINANCE:BTCUSDT"], None, DataServer.ProData)

    # subscribe_bars: invalid server type
    subscribe_bars: Callable[..., object] = client.subscribe_bars
    with pytest.raises(TypeError):
        subscribe_bars(["BINANCE:BTCUSDT"], server="invalid")

    # subscribe_bars: server passed positionally beyond signature
    with pytest.raises(TypeError):
        subscribe_bars(
            ["BINANCE:BTCUSDT"], Interval.OneMinute, None, DataServer.ProData
        )

    # get_fundamental: invalid server type
    get_fundamental: Callable[..., object] = client.get_fundamental
    with pytest.raises(TypeError):
        get_fundamental("AAPL", "NASDAQ", "total_revenue", server="invalid")

    # get_fundamental: server passed positionally beyond signature
    with pytest.raises(TypeError):
        get_fundamental(
            "AAPL",
            "NASDAQ",
            "total_revenue",
            FinancialPeriod.FiscalYear,
            20,
            False,
            DataServer.ProData,
        )
