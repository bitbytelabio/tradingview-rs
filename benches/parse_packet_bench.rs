//! Benchmarks comparing `parse_packet` (manual byte-level parser) vs
//! `_parse_packet` (regex-based parser) for TradingView WebSocket frames.
//!
//! Scenarios:
//!   1. Clean packets (no heartbeats) — measures raw parsing throughput
//!   2. Heartbeat-heavy streams — realistic WebSocket with many `~h~` keepalives
//!   3. Ping-on-wire packets — `~h~` followed by ping digits (e.g. `~h~9999999999`)
//!   4. Mixed workloads — clean + heartbeat + ping, simulating a real session
//!
//! Each scenario is tested at 1K, 10K, and 100K packet counts.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::hint::black_box;
use tradingview::utils::{_parse_packet, parse_packet};

// ---------------------------------------------------------------------------
// Payload fixtures
// ---------------------------------------------------------------------------

/// A realistic `timescale_update` message (from real TradingView traffic).
const TIMESCALE_PAYLOAD: &str = r#"{"m":"timescale_update","p":["cs_1",{"sds_5":{"node":"srv1","s":[{"i":0,"v":[1685633880.0,33019.3,33025.31,33018.84,33021.84,638315.0]},{"i":1,"v":[1685633940.0,33021.96,33023.2,33015.98,33017.97,536590.0]}]}}],"t":1685633880,"t_ms":1685633880000}"#;

/// A realistic `qsd` (quote) message.
const QSD_PAYLOAD: &str = r#"{"m":"qsd","p":[{"n":"AAPL","v":{"bid":150.25,"ask":150.30,"lp":150.28,"volume":12345}}],"t":1685633881,"t_ms":1685633881000}"#;

// ---------------------------------------------------------------------------
// Packet builders
// ---------------------------------------------------------------------------

/// Build `n` consecutive clean `~m~` frames with the given payload.
fn build_clean(payload: &str, n: usize) -> String {
    let header = format!("~m~{}~m~", payload.len());
    let chunk_size = header.len() + payload.len();
    let mut buf = String::with_capacity(chunk_size * n);
    for _ in 0..n {
        buf.push_str(&header);
        buf.push_str(payload);
    }
    buf
}

/// Build `n` frames with a `~h~` heartbeat interleaved between every pair.
/// Layout: `~h~ ~m~<len>~m~<payload> ~h~ ~m~<len>~m~<payload> ...`
fn build_heartbeat_interleaved(payload: &str, n: usize) -> String {
    let header = format!("~m~{}~m~", payload.len());
    let chunk_size = 3 + header.len() + payload.len(); // "~h~" + frame
    let mut buf = String::with_capacity(chunk_size * n);
    for _ in 0..n {
        buf.push_str("~h~");
        buf.push_str(&header);
        buf.push_str(payload);
    }
    buf
}

/// Build `n` frames with a **heavy** heartbeat prefix: each frame is preceded
/// by `~h~` + 10 random-ish ping digits (simulating a `~h~` marker followed
/// by a numeric keepalive payload, as seen in real TradingView traffic).
///
/// Layout: `~h~9999999999 ~m~<len>~m~<payload> ~h~8888888888 ~m~<len>~m~<payload> ...`
fn build_ping_on_wire(payload: &str, n: usize) -> String {
    let header = format!("~m~{}~m~", payload.len());
    // Each cycle: "~h~" (3) + 10 digits + frame
    let chunk_size = 3 + 10 + header.len() + payload.len();
    let mut buf = String::with_capacity(chunk_size * n);
    for i in 0..n {
        // Rotating ping digit to keep the parser honest (not just `~h~` alone).
        let ping = format!("~h~{:010}", i % 1_000_000_000);
        buf.push_str(&ping);
        buf.push_str(&header);
        buf.push_str(payload);
    }
    buf
}

/// Build `n` frames with: 33% clean / 33% heartbeat-interleaved / 33%
/// ping-on-wire — simulating a realistic mixed WebSocket session.
fn build_mixed(payload: &str, n: usize) -> String {
    let header = format!("~m~{}~m~", payload.len());
    let chunk_clean = header.len() + payload.len();
    let chunk_hb = 3 + chunk_clean;
    let chunk_ping = 3 + 10 + chunk_clean;
    let total = (n / 3) * (chunk_clean + chunk_hb + chunk_ping) + 1024; // overestimate
    let mut buf = String::with_capacity(total);

    for i in 0..n {
        match i % 3 {
            0 => {
                // Clean frame
                buf.push_str(&header);
                buf.push_str(payload);
            }
            1 => {
                // Heartbeat-interleaved
                buf.push_str("~h~");
                buf.push_str(&header);
                buf.push_str(payload);
            }
            _ => {
                // Ping-on-wire
                let ping = format!("~h~{:010}", i % 1_000_000_000);
                buf.push_str(&ping);
                buf.push_str(&header);
                buf.push_str(payload);
            }
        }
    }
    buf
}

// ---------------------------------------------------------------------------
// Benchmark groups
// ---------------------------------------------------------------------------

macro_rules! bench_both {
    ($group:expr, $name:expr, $data:expr, $n:expr) => {
        // Manual byte-level parser — clone the data so both parsers get their own.
        let d1 = $data.clone();
        $group.bench_with_input(
            BenchmarkId::new(format!("{}/manual", $name), $n),
            &d1,
            |b, data| b.iter(|| black_box(parse_packet(data))),
        );
        // Regex-based parser
        let d2 = $data.clone();
        $group.bench_with_input(
            BenchmarkId::new(format!("{}/regex", $name), $n),
            &d2,
            |b, data| b.iter(|| black_box(_parse_packet(data))),
        );
    };
}

// --- Scenario 1: clean packets ------------------------------------------

fn bench_clean(c: &mut Criterion) {
    let sizes = [1_000usize, 10_000, 100_000];
    let mut group = c.benchmark_group("ParsePacket/clean");
    for &n in &sizes {
        group.throughput(Throughput::Elements(n as u64));
        let data = build_clean(TIMESCALE_PAYLOAD, n);
        bench_both!(group, "timescale", data, n);
    }
    group.finish();
}

// --- Scenario 2: heartbeat-heavy ----------------------------------------

fn bench_heartbeat(c: &mut Criterion) {
    let sizes = [1_000usize, 10_000, 100_000];
    let mut group = c.benchmark_group("ParsePacket/heartbeat_interleaved");
    for &n in &sizes {
        group.throughput(Throughput::Elements(n as u64));
        let data = build_heartbeat_interleaved(TIMESCALE_PAYLOAD, n);
        bench_both!(group, "timescale_hb", data, n);
    }
    group.finish();
}

// --- Scenario 3: ping-on-wire (~h~ followed by digits) ---------------------

fn bench_ping_on_wire(c: &mut Criterion) {
    let sizes = [1_000usize, 10_000, 100_000];
    let mut group = c.benchmark_group("ParsePacket/ping_on_wire");
    for &n in &sizes {
        group.throughput(Throughput::Elements(n as u64));
        let data = build_ping_on_wire(TIMESCALE_PAYLOAD, n);
        bench_both!(group, "timescale_ping", data, n);
    }
    group.finish();
}

// --- Scenario 4: mixed workload -----------------------------------------

fn bench_mixed(c: &mut Criterion) {
    let sizes = [1_000usize, 10_000, 100_000];
    let mut group = c.benchmark_group("ParsePacket/mixed_workload");
    for &n in &sizes {
        group.throughput(Throughput::Elements(n as u64));
        let data = build_mixed(TIMESCALE_PAYLOAD, n);
        bench_both!(group, "mixed", data, n);
    }
    group.finish();
}

// --- Scenario 5: varied payload sizes -----------------------------------

fn bench_payload_size(c: &mut Criterion) {
    let n = 10_000usize;
    let mut group = c.benchmark_group("ParsePacket/payload_size");
    group.throughput(Throughput::Elements(n as u64));

    // Small payload
    let small = r#"{"m":"qsd","p":[{"n":"A","v":{"lp":1.0}}]}"#;
    let data_small = build_clean(small, n);
    bench_both!(group, "small_json", data_small, n);

    // Medium payload (typical)
    let data_med = build_clean(QSD_PAYLOAD, n);
    bench_both!(group, "medium_json", data_med, n);

    // Large payload (server info + metadata)
    let large = r#"[{"m":"timescale_update","p":["cs_1",{"sds_5":{"s":[]}}],"t":1},{"m":"series_completed","p":["cs_1","s1"],"t":2},{"m":"study_completed","p":["cs_1","st1","st2"],"t":3},{"m":"symbol_resolved","p":["cs_1",{"name":"AAPL","exchange":"NASDAQ","description":"Apple Inc.","type":"stock","session":"extended","timezone":"America/New_York","minmov":1,"pricescale":100,"has_intraday":true,"supported_resolutions":["1","5","15","30","60","D","W","M"]}],"t":4}]"#;
    let data_large = build_clean(large, n);
    bench_both!(group, "large_json", data_large, n);

    group.finish();
}

// --- Scenario 6: heartbeats only (pathological) -------------------------

fn bench_heartbeat_only(c: &mut Criterion) {
    let sizes = [1_000usize, 10_000, 100_000];
    let mut group = c.benchmark_group("ParsePacket/heartbeat_only");
    for &n in &sizes {
        group.throughput(Throughput::Elements(n as u64));
        let data = "~h~".repeat(n);
        group.bench_with_input(BenchmarkId::new("manual", n), &data, |b, data| {
            b.iter(|| black_box(parse_packet(data)))
        });
        group.bench_with_input(BenchmarkId::new("regex", n), &data, |b, data| {
            b.iter(|| black_box(_parse_packet(data)))
        });
    }
    group.finish();
}

// --- Scenario 7: multiple consecutive pings (~h~9999999999 repeated) -----

fn bench_consecutive_pings(c: &mut Criterion) {
    let sizes = [1_000usize, 10_000, 100_000];
    let mut group = c.benchmark_group("ParsePacket/consecutive_pings");
    for &n in &sizes {
        group.throughput(Throughput::Elements(n as u64));
        // N repetitions of "~h~9999999999" with no actual frames — a pathological
        // keepalive-only stream.
        let pattern = "~h~9999999999";
        let data = pattern.repeat(n);
        bench_both!(group, "pings_only", data, n);
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_clean,
    bench_heartbeat,
    bench_ping_on_wire,
    bench_mixed,
    bench_payload_size,
    bench_heartbeat_only,
    bench_consecutive_pings,
);
criterion_main!(benches);
