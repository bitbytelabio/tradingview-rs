"""Unit and integration tests for economic calendar queries (User Story 3)."""

import pytest
from tradingview import (
    EconomicEvent,
    EconomicImportance,
    TradingViewClient,
)


def test_economic_event_model() -> None:
    evt = EconomicEvent(
        id="12345",
        title="Non-Farm Payrolls",
        country="US",
        indicator="NFP",
        ticker="USNFP",
        date=1700000000,
        importance=EconomicImportance.High,
        actual=180000.0,
        forecast=170000.0,
        previous=165000.0,
    )
    assert evt.id == "12345"
    assert evt.title == "Non-Farm Payrolls"
    assert evt.country == "US"
    assert evt.indicator == "NFP"
    assert evt.ticker == "USNFP"
    assert evt.date == 1700000000
    assert evt.importance == EconomicImportance.High
    assert evt.actual == 180000.0
    assert evt.forecast == 170000.0
    assert evt.previous == 165000.0

    d = evt.to_dict()
    assert d["id"] == "12345"
    assert d["importance"] == 1
    assert d["actual"] == 180000.0


@pytest.mark.asyncio
async def test_get_economic_calendar() -> None:
    client = TradingViewClient()
    events = await client.get_economic_calendar(
        countries=["US"],
        min_importance=EconomicImportance.High,
    )

    assert isinstance(events, list)
    for evt in events:
        assert isinstance(evt, EconomicEvent)
        assert evt.country == "US"
        assert evt.importance == EconomicImportance.High

    await client.close()


@pytest.mark.asyncio
async def test_get_economic_calendar_polars_dataframe() -> None:
    import polars as pl

    client = TradingViewClient()
    df = await client.get_economic_calendar(
        countries=["US"],
        min_importance=EconomicImportance.High,
        as_dataframe=True,
    )
    assert isinstance(df, pl.DataFrame)
    assert "id" in df.columns
    assert "country" in df.columns
    assert "importance" in df.columns
    await client.close()
