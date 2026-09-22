"""Unit and integration tests for real-time quote and candle streaming (User Story 2)."""

import asyncio
import pytest
from tradingview import (
    Bar,
    CandleUpdate,
    Interval,
    QuoteTick,
    TradingViewClient,
)

def test_quote_tick_model() -> None:
    tick = QuoteTick(
        symbol="BINANCE:BTCUSDT",
        timestamp=1700000000,
        price=50000.0,
        volume=12.5,
        bid=49999.0,
        ask=50001.0,
        change=250.0,
        change_percent=0.5,
    )
    assert tick.symbol == "BINANCE:BTCUSDT"
    assert tick.timestamp == 1700000000
    assert tick.price == 50000.0
    assert tick.volume == 12.5
    assert tick.bid == 49999.0
    assert tick.ask == 50001.0
    assert tick.change == 250.0
    assert tick.change_percent == 0.5

    d = tick.to_dict()
    assert d["symbol"] == "BINANCE:BTCUSDT"
    assert d["price"] == 50000.0


def test_candle_update_model() -> None:
    candle = CandleUpdate(
        symbol="BINANCE:ETHUSDT",
        interval=Interval.OneMinute,
        timestamp=1700000000,
        open=3000.0,
        high=3010.0,
        low=2995.0,
        close=3005.0,
        volume=500.0,
    )
    assert candle.symbol == "BINANCE:ETHUSDT"
    assert candle.interval == Interval.OneMinute
    assert candle.open == 3000.0
    assert candle.close == 3005.0

    d = candle.to_dict()
    assert d["symbol"] == "BINANCE:ETHUSDT"
    assert d["interval"] == "1"

    bar = candle.to_bar()
    assert isinstance(bar, Bar)
    assert bar.open == 3000.0
    assert bar.close == 3005.0


@pytest.mark.asyncio
async def test_callback_validation_rejects_coroutine() -> None:
    client = TradingViewClient()

    async def async_cb(tick: QuoteTick) -> None:
        pass

    with pytest.raises(TypeError) as excinfo:
        await client.subscribe_quotes(["BINANCE:BTCUSDT"], callback=async_cb)  # type: ignore[arg-type]
    assert "synchronous callable" in str(excinfo.value)
    await client.close()


@pytest.mark.asyncio
async def test_quote_streaming_async_iter_and_callback() -> None:
    client = TradingViewClient()
    received_callbacks: list[QuoteTick] = []

    def on_tick(tick: QuoteTick) -> None:
        received_callbacks.append(tick)

    sub = await client.subscribe_quotes(["BINANCE:BTCUSDT"], callback=on_tick)
    ticks_iter: list[QuoteTick] = []

    try:
        async for tick in sub:
            ticks_iter.append(tick)
            if len(ticks_iter) >= 2:
                break
    finally:
        await sub.stop()
        await client.close()

    assert len(ticks_iter) >= 2
    for t in ticks_iter:
        assert t.symbol == "BINANCE:BTCUSDT"
        assert t.price > 0 or (t.bid is not None and t.bid > 0)


@pytest.mark.asyncio
async def test_candle_streaming_async_iter() -> None:
    client = TradingViewClient()
    sub = await client.subscribe_bars(["BINANCE:BTCUSDT"], interval=Interval.OneMinute)
    candles: list[CandleUpdate] = []

    try:
        async for candle in sub:
            candles.append(candle)
            if len(candles) >= 1:
                break
    finally:
        await sub.stop()
        await client.close()

    assert len(candles) >= 1
    c = candles[0]
    assert c.open > 0
    assert c.close > 0
    assert c.timestamp > 0
