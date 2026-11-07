//! Compare zero and positive range bar counts against the live TradingView service.
//!
//! Run with `cargo run -p tradingview-rs --no-default-features --features rustls-tls
//! --example reproduce_range_bar_count`. Optional positional symbols default to
//! NASDAQ:AAPL, NASDAQ:NDX, and TVC:GOLD. No authentication is required.

use anyhow::{Context, Result, bail, ensure};
use serde_json::{Value, json};
use std::{collections::BTreeSet, sync::Arc, time::Duration};
use tradingview::{
    DataServer, TradingViewDataEvent,
    live::{handler::Handler, websocket::WebSocketClient},
};

struct ProbeHandler(tokio::sync::mpsc::UnboundedSender<(TradingViewDataEvent, Vec<Value>)>);

impl Handler for ProbeHandler {
    fn handle_events(&self, event: TradingViewDataEvent, message: &[Value]) {
        let _ = self.0.send((event, message.to_vec()));
    }

    fn handle_quote_data(&self, _: &[Value]) {}

    fn handle_series_data(&self, _: TradingViewDataEvent, _: &[Value]) {}

    fn notify_error(&self, error: tradingview::Error, message: &[Value]) {
        eprintln!("Transport error: {error:?}; message={message:?}");
    }
}

async fn probe(symbol: &str, from: i64, to: i64, count: u64) -> Result<()> {
    let (sender, mut events) = tokio::sync::mpsc::unbounded_channel();
    let ws = WebSocketClient::builder()
        .auth_token("unauthorized_user_token")
        .server(DataServer::Data)
        .handler(ProbeHandler(sender))
        .build()
        .await?;
    let result = async {
        let session = format!("cs_{}", tradingview::utils::gen_id());
        ws.create_chart_session(&session).await?;
        let resolved = tradingview::utils::symbol_init().instrument(symbol).call()?;
        ws.send("resolve_symbol", &[json!(session), json!("symbol_1"), json!(resolved)]).await?;
        let args = vec![json!(session), json!("sds_1"), json!("s1"), json!("symbol_1"), json!("1D"), json!(count), json!(format!("r,{from}:{to}"))];
        println!("REQUEST symbol={symbol} create_series={}", serde_json::to_string(&args)?);
        ws.send("create_series", &args).await?;
        Arc::clone(&ws).spawn_reader_task();
        let mut points = BTreeSet::new();
        while let Some((event, message)) = events.recv().await {
            match event {
                TradingViewDataEvent::OnChartData | TradingViewDataEvent::OnChartDataUpdate => {
                    let rows = message.get(1).and_then(|value| value.get("sds_1")).and_then(|value| value.get("s")).and_then(Value::as_array).context("missing chart data")?;
                    for row in rows {
                        let open = row.get("v").and_then(|value| value.get(0)).and_then(Value::as_f64).context("missing timestamp")?;
                        points.insert(open as i64);
                    }
                }
                TradingViewDataEvent::OnSeriesCompleted => {
                    ensure!(count > 0, "zero bar count unexpectedly succeeded; re-evaluate the protocol assumption");
                    ensure!(points.len() > count as usize, "expected a multi-year range to return more than {count} bars, got {}", points.len());
                    println!("PASS symbol={symbol} count={count}: series_completed, bars={}, first={:?}, last={:?}", points.len(), points.first(), points.last());
                    return Ok(());
                }
                TradingViewDataEvent::OnError(error) => {
                    let raw = serde_json::to_string(&message)?;
                    println!("RESPONSE symbol={symbol} count={count}: {error:?} {raw}");
                    ensure!(count == 0 && raw.contains("invalid parameters") && raw.contains("create_series"), "unexpected protocol error: {error:?} {raw}");
                    println!("PASS symbol={symbol} count=0: server rejected create_series with invalid parameters");
                    return Ok(());
                }
                _ => {}
            }
        }
        bail!("connection closed before a conclusive response")
    }.await;
    let cleanup = ws.delete().await;
    result?;
    cleanup?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    let symbols = std::env::args().skip(1).collect::<Vec<_>>();
    let symbols = if symbols.is_empty() {
        vec!["NASDAQ:AAPL".into(), "NASDAQ:NDX".into(), "TVC:GOLD".into()]
    } else {
        symbols
    };
    let to = chrono::Utc::now().timestamp();
    let from = to - 3 * 365 * 86_400;
    println!(
        "Live experiment: interval=1D from={from} to={to}; only bar_count changes within each pair"
    );
    for symbol in symbols {
        for count in [0, 512] {
            tokio::time::timeout(Duration::from_secs(45), probe(&symbol, from, to, count))
                .await
                .context("experiment timed out; this is not evidence of protocol rejection")??;
        }
    }
    Ok(())
}
