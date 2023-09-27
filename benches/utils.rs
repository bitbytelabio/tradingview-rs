use criterion::{ black_box, criterion_group, criterion_main, Criterion };
use tradingview::tools::*;
mod data;

fn extract_ohlcv_data_benchmark(c: &mut Criterion) {
    let a30k = black_box(data::generate_chart_random_data(30_000));
    c.bench_function("extract ohlcv data 30k", |b| { b.iter(|| extract_ohlcv_data(&a30k)) });

    let a20k = black_box(data::generate_chart_random_data(20_000));
    c.bench_function("extract ohlcv data 20k", |b| { b.iter(|| extract_ohlcv_data(&a20k)) });

    let a10k = black_box(data::generate_chart_random_data(10_000));
    c.bench_function("extract ohlcv data 10k", |b| { b.iter(|| extract_ohlcv_data(&a10k)) });

    let a5k = black_box(data::generate_chart_random_data(5_000));
    c.bench_function("extract ohlcv data 5k", |b| { b.iter(|| extract_ohlcv_data(&a5k)) });

    let a100 = black_box(data::generate_chart_random_data(100));
    c.bench_function("extract ohlcv data 100", |b| { b.iter(|| extract_ohlcv_data(&a100)) });

    let a1 = black_box(data::generate_chart_random_data(1));
    c.bench_function("extract ohlcv data 1", |b| { b.iter(|| extract_ohlcv_data(&a1)) });
}

fn par_extract_ohlcv_data_benchmark(c: &mut Criterion) {
    let a30k = black_box(data::generate_chart_random_data(30_000));
    c.bench_function("par extract ohlcv data 30k", |b| {
        b.iter(|| par_extract_ohlcv_data(&a30k))
    });

    let a20k = black_box(data::generate_chart_random_data(20_000));
    c.bench_function("par extract ohlcv data 20k", |b| {
        b.iter(|| par_extract_ohlcv_data(&a20k))
    });

    let a10k = black_box(data::generate_chart_random_data(10_000));
    c.bench_function("par extract ohlcv data 10k", |b| {
        b.iter(|| par_extract_ohlcv_data(&a10k))
    });

    let a5k = black_box(data::generate_chart_random_data(5_000));
    c.bench_function("par extract ohlcv data 5k", |b| { b.iter(|| par_extract_ohlcv_data(&a5k)) });

    let a100 = black_box(data::generate_chart_random_data(100));
    c.bench_function("par extract ohlcv data 100", |b| {
        b.iter(|| par_extract_ohlcv_data(&a100))
    });

    let a1 = black_box(data::generate_chart_random_data(1));
    c.bench_function("par extract ohlcv data 1", |b| { b.iter(|| par_extract_ohlcv_data(&a1)) });
}

criterion_group!(benches, extract_ohlcv_data_benchmark, par_extract_ohlcv_data_benchmark);
criterion_main!(benches);
