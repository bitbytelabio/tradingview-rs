"""Unit and integration tests for historical data retrieval (User Story 1)."""

import pytest
from tradingview import Bar, HistoricalSeries, Interval, TradingViewClient


def test_bar_model_attributes_and_conversion() -> None:
    bar = Bar(
        timestamp=1700000000,
        open=150.0,
        high=155.0,
        low=149.0,
        close=154.0,
        volume=1000000.0,
    )
    assert bar.timestamp == 1700000000
    assert bar.open == 150.0
    assert bar.high == 155.0
    assert bar.low == 149.0
    assert bar.close == 154.0
    assert bar.volume == 1000000.0

    d = bar.to_dict()
    assert d == {
        "timestamp": 1700000000,
        "open": 150.0,
        "high": 155.0,
        "low": 149.0,
        "close": 154.0,
        "volume": 1000000.0,
    }

    t = bar.to_tuple()
    assert t == (1700000000, 150.0, 155.0, 149.0, 154.0, 1000000.0)


def test_historical_series_methods() -> None:
    b1 = Bar(1700000000, 100.0, 105.0, 95.0, 102.0, 1000.0)
    b2 = Bar(1700000060, 102.0, 108.0, 101.0, 107.0, 2000.0)
    series = HistoricalSeries("AAPL", "NASDAQ", Interval.OneMinute, [b1, b2])

    assert len(series) == 2
    assert series[0] == b1
    assert series[-1] == b2

    with pytest.raises(IndexError):
        _ = series[10]

    # Iteration
    bars = list(series)
    assert bars == [b1, b2]

    # Dict export
    d = series.to_dict()
    assert d["timestamp"] == [1700000000, 1700000060]
    assert d["close"] == [102.0, 107.0]

    # Polars & Pandas export
    try:
        import polars as pl

        df_pl = series.to_polars()
        assert isinstance(df_pl, pl.DataFrame)
        assert df_pl.shape == (2, 6)
    except ImportError:
        pass

    try:
        import pandas as pd

        df_pd = series.to_pandas()
        assert isinstance(df_pd, pd.DataFrame)
        assert df_pd.shape == (2, 6)
    except ImportError:
        pass


@pytest.mark.asyncio
async def test_get_historical_single_symbol() -> None:
    client = TradingViewClient()
    series = await client.get_historical("AAPL", "NASDAQ", Interval.OneDay, n_bars=10)
    assert series.symbol == "AAPL"
    assert series.exchange == "NASDAQ"
    assert len(series) > 0
    assert len(series) <= 10

    bar = series[0]
    assert bar.timestamp > 0
    assert bar.open > 0
    assert bar.close > 0
    await client.close()


@pytest.mark.asyncio
async def test_get_historical_batch() -> None:
    client = TradingViewClient()
    batch = await client.get_historical_batch(
        [("AAPL", "NASDAQ"), ("MSFT", "NASDAQ")],
        interval=Interval.OneDay,
        n_bars=5,
        max_concurrency=2,
    )
    assert "NASDAQ:AAPL" in batch
    assert "NASDAQ:MSFT" in batch
    assert len(batch["NASDAQ:AAPL"]) > 0
    assert len(batch["NASDAQ:MSFT"]) > 0
    await client.close()


@pytest.mark.asyncio
async def test_get_historical_polars_dataframe() -> None:
    import polars as pl

    client = TradingViewClient()

    # Using as_dataframe=True
    df = await client.get_historical(
        "AAPL", "NASDAQ", Interval.OneDay, n_bars=10, as_dataframe=True
    )
    assert isinstance(df, pl.DataFrame)
    assert df.height == 10
    assert set(df.columns) == {"timestamp", "open", "high", "low", "close", "volume"}
    min_val = df["close"].min()
    assert min_val is not None and float(str(min_val)) > 0

    # Using convenience method get_historical_df
    df2 = await client.get_historical_df("MSFT", "NASDAQ", Interval.OneDay, n_bars=5)
    assert isinstance(df2, pl.DataFrame)
    assert df2.height == 5
    assert set(df2.columns) == {"timestamp", "open", "high", "low", "close", "volume"}

    await client.close()


@pytest.mark.asyncio
async def test_get_historical_batch_polars_dataframe() -> None:
    import polars as pl

    client = TradingViewClient()

    batch_df = await client.get_historical_batch(
        [("AAPL", "NASDAQ"), ("MSFT", "NASDAQ")],
        interval=Interval.OneDay,
        n_bars=5,
        as_dataframe=True,
    )
    assert isinstance(batch_df, dict)
    assert "NASDAQ:AAPL" in batch_df and "NASDAQ:MSFT" in batch_df
    assert isinstance(batch_df["NASDAQ:AAPL"], pl.DataFrame)
    assert isinstance(batch_df["NASDAQ:MSFT"], pl.DataFrame)
    assert batch_df["NASDAQ:AAPL"].height == 5
    assert batch_df["NASDAQ:MSFT"].height == 5

    await client.close()
