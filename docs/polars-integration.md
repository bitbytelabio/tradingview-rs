# Polars DataFrame Integration Guide

The `tradingview` Python library provides native zero-copy and column-oriented integration with [Polars](https://pola.rs), enabling institutional-grade quantitative workflows and vectorized technical analysis.

---

## Why Polars?

- **Vectorized Performance**: Written in Rust with Apache Arrow memory layout, Polars executes columnar calculations $10\times$–$50\times$ faster than traditional row-oriented libraries.
- **Lazy Evaluation**: Construct full trading strategies and indicator pipelines using `.lazy()` queries with query optimization before execution.
- **Memory Efficiency**: Column-oriented contiguous arrays eliminate Python object overhead for large market data time series.

---

## Direct DataFrame Retrieval

Set `as_dataframe=True` on any client retrieval method to obtain a `polars.DataFrame` immediately:

```python
import asyncio
import polars as pl
from tradingview import TradingViewClient, Interval, FinancialPeriod, EconomicImportance

async def main():
    client = TradingViewClient()

    # 1. Historical Candlesticks
    df_bars: pl.DataFrame = await client.get_historical(
        "AAPL", "NASDAQ", Interval.OneDay, n_bars=250, as_dataframe=True
    )
    print("Historical Bars Schema:")
    print(df_bars.schema)
    # Schema: timestamp: Int64, open: Float64, high: Float64, low: Float64, close: Float64, volume: Float64

    # 2. Batch Retrieval
    batch: dict[str, pl.DataFrame] = await client.get_historical_batch(
        [("AAPL", "NASDAQ"), ("MSFT", "NASDAQ")],
        interval=Interval.OneDay,
        n_bars=100,
        as_dataframe=True,
    )
    aapl_df = batch["NASDAQ:AAPL"]
    msft_df = batch["NASDAQ:MSFT"]

    # 3. Corporate Fundamentals
    df_fund: pl.DataFrame = await client.get_fundamental(
        "AAPL", "NASDAQ", "total_revenue", FinancialPeriod.FiscalYear, n_bars=10, as_dataframe=True
    )
    # Columns: symbol, exchange, fund_id, period, timestamp, value, index

    # 4. Economic Calendar
    df_calendar: pl.DataFrame = await client.get_economic_calendar(
        countries=["US"], min_importance=EconomicImportance.High, as_dataframe=True
    )

    await client.close()

asyncio.run(main())
```

---

## Technical Indicators on Polars DataFrames

With Polars expressions, standard technical indicators can be calculated in microseconds without writing manual loops:

```python
import polars as pl

def compute_indicators(df: pl.DataFrame) -> pl.DataFrame:
    """Computes SMA, EMA, Bollinger Bands, and Returns in vectorized expressions."""
    return df.with_columns([
        # Convert timestamp to human-readable datetime
        pl.from_epoch("timestamp", time_unit="s").alias("datetime"),
        
        # Simple Moving Averages
        pl.col("close").rolling_mean(window_size=20).alias("sma_20"),
        pl.col("close").rolling_mean(window_size=50).alias("sma_50"),
        
        # Exponential Moving Average
        pl.col("close").ewm_mean(span=20).alias("ema_20"),
        
        # Bollinger Bands (20 periods, 2 standard deviations)
        (pl.col("close").rolling_mean(window_size=20) + 2 * pl.col("close").rolling_std(window_size=20)).alias("bb_upper"),
        (pl.col("close").rolling_mean(window_size=20) - 2 * pl.col("close").rolling_std(window_size=20)).alias("bb_lower"),
        
        # Percentage Returns
        pl.col("close").pct_change().alias("daily_return"),
        
        # True Range
        pl.max_horizontal([
            pl.col("high") - pl.col("low"),
            (pl.col("high") - pl.col("close").shift(1)).abs(),
            (pl.col("low") - pl.col("close").shift(1)).abs(),
        ]).alias("true_range"),
    ])
```

---

## Lazy Execution for Large Backtests

```python
import polars as pl

def backtest_sma_crossover(df: pl.DataFrame) -> pl.DataFrame:
    """Optimized vectorized backtest using Polars LazyFrame."""
    return (
        df.lazy()
        .with_columns([
            pl.col("close").rolling_mean(window_size=10).alias("fast_ma"),
            pl.col("close").rolling_mean(window_size=30).alias("slow_ma"),
        ])
        .with_columns([
            pl.when(pl.col("fast_ma") > pl.col("slow_ma"))
            .then(1)
            .otherwise(0)
            .alias("signal")
        ])
        .with_columns([
            (pl.col("signal").shift(1) * pl.col("close").pct_change()).alias("strategy_return")
        ])
        .collect()
    )
```
