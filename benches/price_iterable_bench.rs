//! Benchmarks for `PriceIterable::to_vec()` — comparing the fixed borrow-based
//! `ChartHistoricalData` implementation against the `Vec<DataPoint>` baseline.
//!
//! The old implementation of `ChartHistoricalData::to_vec()` called
//! `self.data.to_vec()`, which cloned the entire `Vec<DataPoint>` (O(n)
//! allocation + O(n) clone).  The fix replaces it with `self.data.iter()`,
//! which borrows references with zero allocations.
//!
//! Sizes: 1,000 / 10,000 / 100,000 bars.
//! Each benchmark iterates all OHLCV fields to simulate realistic downstream
//! consumption.

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::hint::black_box;
use tradingview::chart::{ChartHistoricalData, ChartOptions, DataPoint, OHLCV, PriceIterable};
use tradingview::websocket::SeriesInfo;
use ustr::Ustr;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a `DataPoint` with valid OHLCV data.
fn dp(ts: i64, o: f64, h: f64, l: f64, c: f64, v: f64) -> DataPoint {
    DataPoint {
        index: 0,
        value: vec![ts as f64, o, h, l, c, v],
    }
}

/// Generate `n` consecutive daily candles.
fn make_candles(n: usize) -> Vec<DataPoint> {
    let base_ts = 1_700_000_000i64;
    (0..n)
        .map(|i| {
            let ts = base_ts + i as i64 * 86_400;
            let o = 100.0 + i as f64;
            dp(ts, o, o + 1.0, o - 1.0, o + 1.0, 1000.0 + i as f64)
        })
        .collect()
}

/// Build a `ChartHistoricalData` with the given candles.
fn make_chart(data: Vec<DataPoint>) -> ChartHistoricalData {
    let options = ChartOptions::builder()
        .instrument("NASDAQ:AAPL")
        .build()
        .expect("valid ChartOptions for bench");
    ChartHistoricalData {
        symbol_info: Default::default(),
        series_info: SeriesInfo {
            chart_session: Ustr::default(),
            options,
        },
        data,
    }
}

// ---------------------------------------------------------------------------
// Workload: iterate every OHLCV accessor (simulates real downstream usage)
// ---------------------------------------------------------------------------

fn iterate_all_fields(data: &impl PriceIterable<Item = DataPoint>) {
    let c: f64 = data.closes().sum();
    let o: f64 = data.opens().sum();
    let h: f64 = data.highs().sum();
    let l: f64 = data.lows().sum();
    let v: f64 = data.volumes().sum();
    let ts: i64 = data.timestamps().sum();
    black_box((c, o, h, l, v, ts));
}

// ---------------------------------------------------------------------------
// Benchmark groups
// ---------------------------------------------------------------------------

/// `Vec<DataPoint>` — the baseline (always used `iter()` — no clone).
fn bench_vec_direct(c: &mut Criterion) {
    let sizes = [1_000usize, 10_000, 100_000];
    let mut group = c.benchmark_group("PriceIterable/Vec-direct-baseline");
    for &n in &sizes {
        group.throughput(Throughput::Elements(n as u64));
        let candles = make_candles(n);
        group.bench_with_input(BenchmarkId::new("all_fields", n), &candles, |b, data| {
            b.iter(|| iterate_all_fields(data));
        });
    }
    group.finish();
}

/// `ChartHistoricalData` — the **fixed** implementation (`self.data.iter()`).
fn bench_chart_fixed(c: &mut Criterion) {
    let sizes = [1_000usize, 10_000, 100_000];
    let mut group = c.benchmark_group("PriceIterable/ChartHistorical-fixed");
    for &n in &sizes {
        group.throughput(Throughput::Elements(n as u64));
        let chart = make_chart(make_candles(n));
        group.bench_with_input(BenchmarkId::new("all_fields", n), &chart, |b, chart| {
            b.iter(|| iterate_all_fields(chart));
        });
    }
    group.finish();
}

/// Simulates the **old** clone-based path: clone the underlying `Vec<DataPoint>`,
/// then iterate over all OHLCV fields.  This shows what the old
/// `ChartHistoricalData::to_vec()` was doing internally before the fix.
fn bench_old_clone_based(c: &mut Criterion) {
    let sizes = [1_000usize, 10_000, 100_000];
    let mut group = c.benchmark_group("PriceIterable/OLD-clone-then-iterate");
    for &n in &sizes {
        group.throughput(Throughput::Elements(n as u64));
        let candles = make_candles(n);
        group.bench_with_input(BenchmarkId::new("clone+iterate", n), &candles, |b, data| {
            b.iter(|| {
                // Old behaviour: clone the entire Vec, then iterate.
                let cloned: Vec<DataPoint> = data.iter().cloned().collect();
                for dp in &cloned {
                    black_box(dp.open());
                    black_box(dp.high());
                    black_box(dp.low());
                    black_box(dp.close());
                    black_box(dp.volume());
                    black_box(dp.timestamp());
                }
                black_box(cloned);
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_vec_direct,
    bench_chart_fixed,
    bench_old_clone_based,
);
criterion_main!(benches);
