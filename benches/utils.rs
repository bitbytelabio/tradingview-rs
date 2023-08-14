use criterion::{black_box, criterion_group, criterion_main, Criterion};
use tradingview_rs::tools::*;
mod data;

fn extract_ohlcv_data_benchmark(c: &mut Criterion) {
    let mut a30k = black_box(data::generate_chart_random_data(30_000));
    c.bench_function("extract ohlcv data 30k", |b| {
        b.iter(|| extract_ohlcv_data(&mut a30k))
    });

    let mut a20k = black_box(data::generate_chart_random_data(20_000));
    c.bench_function("extract ohlcv data 20k", |b| {
        b.iter(|| extract_ohlcv_data(&mut a20k))
    });

    let mut a10k = black_box(data::generate_chart_random_data(10_000));
    c.bench_function("extract ohlcv data 10k", |b| {
        b.iter(|| extract_ohlcv_data(&mut a10k))
    });

    let mut a5k = black_box(data::generate_chart_random_data(5_000));
    c.bench_function("extract ohlcv data 5k", |b| {
        b.iter(|| extract_ohlcv_data(&mut a5k))
    });

    let mut a100 = black_box(data::generate_chart_random_data(5_000));
    c.bench_function("extract ohlcv data 100", |b| {
        b.iter(|| extract_ohlcv_data(&mut a100))
    });

    let mut a1 = black_box(data::generate_chart_random_data(5_000));
    c.bench_function("extract ohlcv data 1", |b| {
        b.iter(|| extract_ohlcv_data(&mut a1))
    });
}

fn par_extract_ohlcv_data_benchmark(c: &mut Criterion) {
    let mut a30k = black_box(data::generate_chart_random_data(30_000));
    c.bench_function("par extract ohlcv data 30k", |b| {
        b.iter(|| par_extract_ohlcv_data(&mut a30k))
    });

    let mut a20k = black_box(data::generate_chart_random_data(20_000));
    c.bench_function("par extract ohlcv data 20k", |b| {
        b.iter(|| par_extract_ohlcv_data(&mut a20k))
    });

    let mut a10k = black_box(data::generate_chart_random_data(10_000));
    c.bench_function("par extract ohlcv data 10k", |b| {
        b.iter(|| par_extract_ohlcv_data(&mut a10k))
    });

    let mut a5k = black_box(data::generate_chart_random_data(5_000));
    c.bench_function("par extract ohlcv data 5k", |b| {
        b.iter(|| par_extract_ohlcv_data(&mut a5k))
    });

    let mut a100 = black_box(data::generate_chart_random_data(5_000));
    c.bench_function("par extract ohlcv data 100", |b| {
        b.iter(|| par_extract_ohlcv_data(&mut a100))
    });

    let mut a1 = black_box(data::generate_chart_random_data(5_000));
    c.bench_function("par extract ohlcv data 1", |b| {
        b.iter(|| par_extract_ohlcv_data(&mut a1))
    });
}

criterion_group!(
    benches,
    extract_ohlcv_data_benchmark,
    par_extract_ohlcv_data_benchmark
);
criterion_main!(benches);
