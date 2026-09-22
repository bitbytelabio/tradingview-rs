//! Criterion benchmarks for `tradingview-rs` performance baselines and regressions.
//!
//! Covers:
//! - `PriceIterable::to_vec()`: zero-allocation borrowed iteration vs clone-and-iterate.
//! - `Utils/gen_id`: random alphanumeric session ID generation throughput.
//! - `HttpClient`: shared `LazyLock` client acquisition vs legacy per-call client creation.
//! - `Utils/parse_packet`: realistic frame batches (1, 5, 20 packets) alongside fixture-scale (1K, 10K).
//! - `Utils/format_packet`: packet serialization & framing (`SocketMessageSer`).
//! - `WebSocket/write-path-channel`: burst mpsc send throughput with a shared runtime outside iteration.

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use serde_json::json;
use std::hint::black_box;
use tradingview::chart::{ChartHistoricalData, ChartOptions, DataPoint, PriceIterable};
use tradingview::live::models::SocketMessageSer;
use tradingview::utils::gen_id;
#[allow(deprecated)]
use tradingview::utils::{build_request, http_client};
use tradingview::utils::{format_packet, parse_packet};
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

/// `Vec<DataPoint>` direct baseline: zero-allocation borrowed iteration over all OHLCV fields.
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

/// `ChartHistoricalData` fixed implementation: zero-allocation borrowed iteration (`self.data.iter()`).
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

/// Simulates the old clone-based `ChartHistoricalData::to_vec()` path:
/// clone the entire `Vec<DataPoint>` first, then iterate over all OHLCV fields
/// using the exact same `iterate_all_fields` workload for equivalent comparison.
fn bench_old_clone_based(c: &mut Criterion) {
    let sizes = [1_000usize, 10_000, 100_000];
    let mut group = c.benchmark_group("PriceIterable/OLD-clone-then-iterate");
    for &n in &sizes {
        group.throughput(Throughput::Elements(n as u64));
        let candles = make_candles(n);
        group.bench_with_input(BenchmarkId::new("clone+iterate", n), &candles, |b, data| {
            b.iter(|| {
                let cloned: Vec<DataPoint> = data.clone();
                iterate_all_fields(&cloned);
                black_box(cloned);
            });
        });
    }
    group.finish();
}

/// Benchmark `gen_id()` — measures ID generation throughput.
fn bench_gen_id(c: &mut Criterion) {
    let mut group = c.benchmark_group("Utils/gen_id");
    group.throughput(Throughput::Elements(1));
    group.bench_function("generate", |b| {
        b.iter(|| black_box(gen_id()));
    });
    group.finish();
}

/// Benchmark HTTP client acquisition.
///
/// Compares the old `build_request(None)` (creates a fresh client every call)
/// against the new `http_client()` (clones the shared `LazyLock` client —
/// effectively an `Arc` increment).
///
/// Also benchmarks 100 sequential acquisitions to simulate request-heavy
/// workloads.
#[allow(deprecated)]
fn bench_http_client(c: &mut Criterion) {
    let mut group = c.benchmark_group("HttpClient");

    // Single acquisition — old vs new
    group.bench_function("old_build_request_none", |b| {
        b.iter(|| {
            #[allow(deprecated)]
            black_box(build_request(None).unwrap());
        });
    });

    group.bench_function("new_http_client", |b| {
        b.iter(|| {
            black_box(http_client());
        });
    });

    // 100 sequential acquisitions — simulates 100 sequential requests
    group.bench_function("old_100_sequential", |b| {
        b.iter(|| {
            for _ in 0..100 {
                #[allow(deprecated)]
                black_box(build_request(None).unwrap());
            }
        });
    });

    group.bench_function("new_100_sequential", |b| {
        b.iter(|| {
            for _ in 0..100 {
                black_box(http_client());
            }
        });
    });

    group.finish();
}

/// Build synthetic TradingView WebSocket packets for benchmarking.
/// Format: `~m~<len>~m~<JSON payload>` repeated `n` times.
fn build_packets(n: usize) -> String {
    let payload = r#"{"m":"timescale_update","p":["cs_1",{"s":[{"i":0,"v":[1685633880.0,100.0,105.0,99.0,102.0,1000.0]}]}]}"#;
    let header = format!("~m~{}~m~", payload.len());
    let mut buf = String::with_capacity((header.len() + payload.len()) * n);
    for _ in 0..n {
        buf.push_str(&header);
        buf.push_str(payload);
    }
    buf
}

/// Benchmark `parse_packet()` across both realistic packet batches (1, 5, 20)
/// and large fixture-scale counts (1K, 10K).
fn bench_parse_packet(c: &mut Criterion) {
    // Realistic small batches typical of live WebSocket message arrivals
    let realistic_sizes = [1usize, 5, 20];
    let mut realistic_group = c.benchmark_group("Utils/parse_packet/realistic");
    for &n in &realistic_sizes {
        let data = build_packets(n);
        realistic_group.throughput(Throughput::Bytes(data.len() as u64));
        realistic_group.bench_with_input(BenchmarkId::new("packets", n), &data, |b, data| {
            b.iter(|| black_box(parse_packet(data)));
        });
    }
    realistic_group.finish();

    // Fixture-scale counts simulating large historical or replay backfills
    let fixture_sizes = [1_000usize, 10_000];
    let mut fixture_group = c.benchmark_group("Utils/parse_packet/fixture_scale");
    for &n in &fixture_sizes {
        let data = build_packets(n);
        fixture_group.throughput(Throughput::Bytes(data.len() as u64));
        fixture_group.bench_with_input(BenchmarkId::new("packets", n), &data, |b, data| {
            b.iter(|| black_box(parse_packet(data)));
        });
    }
    fixture_group.finish();
}

/// Benchmark `format_packet()` serialization and framing throughput using `SocketMessageSer`.
fn bench_format_packet(c: &mut Criterion) {
    let mut group = c.benchmark_group("Utils/format_packet");
    group.throughput(Throughput::Elements(1));

    let heartbeat_json_cmd = SocketMessageSer::new("~h~5", json!([]));
    group.bench_function("heartbeat_json_cmd", |b| {
        b.iter(|| black_box(format_packet(&heartbeat_json_cmd).unwrap()));
    });

    let quote_sub = SocketMessageSer::new(
        "quote_add_symbols",
        json!(["qs_session_1", "BINANCE:BTCUSDT", "NASDAQ:AAPL"]),
    );
    group.bench_function("command_payload", |b| {
        b.iter(|| black_box(format_packet(&quote_sub).unwrap()));
    });

    group.finish();
}

/// Benchmark WebSocket write-path throughput using an mpsc channel.
///
/// Keeps a single shared Tokio runtime outside `b.iter()` to measure pure channel
/// send throughput without runtime instantiation overhead inside the timing loop.
fn bench_ws_write_path(c: &mut Criterion) {
    use tokio::runtime::Runtime;
    use tokio::sync::mpsc;

    let rt = Runtime::new().expect("create tokio runtime for bench");
    let sizes = [100usize, 1_000, 10_000];

    let mut group = c.benchmark_group("WebSocket/write-path-channel");
    for &n in &sizes {
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::new("send_n_messages", n), &n, |b, &n| {
            b.iter(|| {
                rt.block_on(async {
                    let (tx, mut rx) = mpsc::channel::<String>(1024);
                    let consumer = tokio::spawn(async move { while rx.recv().await.is_some() {} });
                    for i in 0..n {
                        let _ = tx.send(format!("msg_{i}")).await;
                    }
                    drop(tx);
                    let _ = consumer.await;
                });
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
    bench_gen_id,
    bench_http_client,
    bench_parse_packet,
    bench_format_packet,
    bench_ws_write_path,
);
criterion_main!(benches);
