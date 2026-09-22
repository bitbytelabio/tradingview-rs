"""Unit and integration tests for fundamental metric queries (User Story 3)."""

import pytest
from tradingview import (
    FinancialPeriod,
    FundamentalPoint,
    FundamentalSeries,
    TradingViewClient,
)


def test_fundamental_point_model() -> None:
    p = FundamentalPoint(timestamp=1700000000, value=1234567.89, index=1)
    assert p.timestamp == 1700000000
    assert p.value == 1234567.89
    assert p.index == 1

    d = p.to_dict()
    assert d == {
        "timestamp": 1700000000,
        "value": 1234567.89,
        "index": 1,
    }


def test_fundamental_series_methods() -> None:
    p1 = FundamentalPoint(1600000000, 100.0, 0)
    p2 = FundamentalPoint(1700000000, 150.0, 1)
    series = FundamentalSeries(
        "AAPL", "NASDAQ", "total_revenue", FinancialPeriod.FiscalYear, [p1, p2]
    )

    assert len(series) == 2
    assert series[0] == p1
    assert series[-1] == p2

    with pytest.raises(IndexError):
        _ = series[10]

    points = list(series)
    assert points == [p1, p2]

    d = series.to_dict()
    assert d["symbol"] == "AAPL"
    assert d["fund_id"] == "total_revenue"
    assert d["period"] == "FY"
    assert d["value"] == [100.0, 150.0]


@pytest.mark.asyncio
async def test_get_fundamental() -> None:
    client = TradingViewClient()
    fund = await client.get_fundamental(
        "AAPL", "NASDAQ", "total_revenue", FinancialPeriod.FiscalYear, n_bars=3
    )

    assert fund.symbol == "AAPL"
    assert fund.exchange == "NASDAQ"
    assert fund.fund_id == "total_revenue"
    assert fund.period == FinancialPeriod.FiscalYear
    assert len(fund.points) > 0

    p = fund.points[0]
    assert p.timestamp > 0
    assert p.value > 0

    await client.close()


@pytest.mark.asyncio
async def test_get_fundamental_polars_dataframe() -> None:
    import polars as pl

    client = TradingViewClient()
    df = await client.get_fundamental(
        "AAPL",
        "NASDAQ",
        "total_revenue",
        FinancialPeriod.FiscalYear,
        n_bars=3,
        as_dataframe=True,
    )
    assert isinstance(df, pl.DataFrame)
    assert df.height > 0
    assert {"timestamp", "value", "index"}.issubset(set(df.columns))
    await client.close()
