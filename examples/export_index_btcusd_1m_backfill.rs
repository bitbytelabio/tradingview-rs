use anyhow::{Context, Result, anyhow};
use chrono::{Datelike, Duration as ChronoDuration, TimeZone, Utc};
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    io::Write,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::mpsc;
use tradingview::{
    ChartOptions, DataPoint, Interval, OHLCV, UserCookies,
    client::misc::get_quote_token,
    live::{
        handler::{
            command::CommandRunner,
            message::{Command, TradingViewResponse},
        },
        models::DataServer,
        websocket::WebSocketClient,
    },
};

#[derive(Debug)]
struct Config {
    symbol: String,
    exchange: String,
    out_dir: PathBuf,
    years_back: i64,
    from_date: Option<chrono::NaiveDate>,
    to_date: Option<chrono::NaiveDate>,
    interval: Interval,
    timeframe_label: String,
    step_seconds: i64,
}

fn parse_timeframe(tf: &str) -> Result<(Interval, String, i64)> {
    match tf {
        "1m" => Ok((Interval::OneMinute, "1m".to_string(), 60)),
        "1h" => Ok((Interval::OneHour, "1h".to_string(), 3600)),
        "1D" | "1d" => Ok((Interval::OneDay, "1D".to_string(), 86_400)),
        _ => Err(anyhow!(
            "unsupported --tf '{}'; expected one of: 1m, 1h, 1D",
            tf
        )),
    }
}

fn parse_yyyy_mm_dd(name: &str, value: &str) -> Result<chrono::NaiveDate> {
    chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .with_context(|| format!("{name} must be YYYY-MM-DD, got '{value}'"))
}

fn parse_args() -> Result<Config> {
    let mut symbol = "BTCUSD".to_string();
    let mut exchange = "INDEX".to_string();
    let mut out_dir = PathBuf::from("../../csv_exports_backfill");
    let mut years_back = 2_i64;
    let mut from_date: Option<chrono::NaiveDate> = None;
    let mut to_date: Option<chrono::NaiveDate> = None;
    let mut tf = "1m".to_string();

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--symbol" => {
                symbol = args
                    .next()
                    .ok_or_else(|| anyhow!("missing value for --symbol"))?
            }
            "--exchange" => {
                exchange = args
                    .next()
                    .ok_or_else(|| anyhow!("missing value for --exchange"))?
            }
            "--out-dir" => {
                out_dir = PathBuf::from(
                    args.next()
                        .ok_or_else(|| anyhow!("missing value for --out-dir"))?,
                )
            }
            "--years-back" => {
                years_back = args
                    .next()
                    .ok_or_else(|| anyhow!("missing value for --years-back"))?
                    .parse::<i64>()
                    .context("--years-back must be an integer")?
            }
            "--from" => {
                let v = args
                    .next()
                    .ok_or_else(|| anyhow!("missing value for --from"))?;
                from_date = Some(parse_yyyy_mm_dd("--from", &v)?);
            }
            "--to" => {
                let v = args
                    .next()
                    .ok_or_else(|| anyhow!("missing value for --to"))?;
                to_date = Some(parse_yyyy_mm_dd("--to", &v)?);
            }
            "--tf" => {
                tf = args
                    .next()
                    .ok_or_else(|| anyhow!("missing value for --tf"))?
            }
            "-h" | "--help" => {
                println!(
                    "Usage: cargo run --example export_index_btcusd_1m_backfill -- \\\n  [--symbol BTCUSD] [--exchange INDEX] [--tf 1m|1h|1D] \\\n  [--from YYYY-MM-DD] [--to YYYY-MM-DD] \\\n  [--years-back 2] [--out-dir ../../csv_exports_backfill]\n\n\
Notes:\n  - If --from/--to are omitted, program backfills from now to --years-back.\n  - If --from is set without --to, --to defaults to now.\n  - If --to is set without --from, --from defaults to now - --years-back."
                );
                std::process::exit(0);
            }
            other => return Err(anyhow!("unknown argument: {other}")),
        }
    }

    let (interval, timeframe_label, step_seconds) = parse_timeframe(&tf)?;

    Ok(Config {
        symbol,
        exchange,
        out_dir,
        years_back,
        from_date,
        to_date,
        interval,
        timeframe_label,
        step_seconds,
    })
}

async fn resolve_auth_token() -> Result<String> {
    if let Ok(token) = env::var("TV_TOKEN") {
        if !token.trim().is_empty() {
            return Ok(token);
        }
    }

    if let Ok(token) = env::var("TV_AUTH_TOKEN") {
        if !token.trim().is_empty() {
            return Ok(token);
        }
    }

    let username = env::var("TV_USERNAME").context("TV_USERNAME is not set")?;
    let password = env::var("TV_PASSWORD").context("TV_PASSWORD is not set")?;
    let totp_secret = env::var("TV_TOTP_SECRET").ok();

    let mut user = UserCookies::default();
    let user = user
        .login(&username, &password, totp_secret.as_deref())
        .await
        .context("failed to login to TradingView")?;

    if !user.auth_token.trim().is_empty() {
        return Ok(user.auth_token);
    }

    let token = get_quote_token(&user)
        .await
        .context("failed to get quote token")?;
    Ok(token.trim_matches('"').to_string())
}

async fn fetch_window(
    auth_token: &str,
    exchange: &str,
    symbol: &str,
    interval: Interval,
    replay_from: i64,
) -> Result<Vec<DataPoint>> {
    let (data_tx, mut data_rx) = mpsc::unbounded_channel();
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();

    let ws = WebSocketClient::builder()
        .auth_token(auth_token)
        .server(DataServer::ProData)
        .data_tx(data_tx)
        .build()
        .await
        .context("failed to create websocket client")?;

    let command_runner = CommandRunner::new(cmd_rx, Arc::clone(&ws));
    let shutdown = command_runner.shutdown_token();

    let runner_handle = tokio::spawn(async move {
        if let Err(err) = command_runner.run().await {
            eprintln!("command runner error: {err:?}");
        }
    });

    let options = ChartOptions::builder()
        .symbol(symbol.into())
        .exchange(exchange.into())
        .interval(interval)
        .bar_count(50_000)
        .replay_mode(true)
        .replay_from(replay_from)
        .build();

    cmd_tx
        .send(Command::set_market(options))
        .map_err(|_| anyhow!("failed to send set_market command"))?;

    cmd_tx
        .send(Command::CreateQuoteSession)
        .map_err(|_| anyhow!("failed to send create_quote_session command"))?;

    cmd_tx
        .send(Command::SetQuoteFields)
        .map_err(|_| anyhow!("failed to send set_quote_fields command"))?;

    let mut data = Vec::new();

    loop {
        let next = tokio::time::timeout(Duration::from_secs(30), data_rx.recv()).await;
        let response = match next {
            Ok(Some(r)) => r,
            Ok(None) => return Err(anyhow!("data channel closed unexpectedly")),
            Err(_) => return Err(anyhow!("timeout waiting for TradingView response")),
        };

        match response {
            TradingViewResponse::ChartData(_series_info, points) => {
                data.extend(points);
            }
            TradingViewResponse::SeriesCompleted(_message) => {
                break;
            }
            TradingViewResponse::Error(err, message) => {
                return Err(anyhow!(
                    "tradingview error: {:?}, message context: {:?}",
                    err,
                    message
                ));
            }
            _ => {}
        }
    }

    shutdown.cancel();
    let _ = tokio::time::timeout(Duration::from_secs(5), runner_handle).await;

    data.sort_by_key(|d| d.timestamp());
    data.dedup_by_key(|d| d.timestamp());

    Ok(data)
}

fn write_monthly_csvs(
    out_dir: &PathBuf,
    exchange: &str,
    symbol: &str,
    timeframe_label: &str,
    bars: &[DataPoint],
) -> Result<()> {
    fs::create_dir_all(out_dir)?;

    let mut buckets: BTreeMap<(i32, u32), Vec<&DataPoint>> = BTreeMap::new();
    for bar in bars {
        let dt = bar.datetime();
        buckets
            .entry((dt.year(), dt.month()))
            .or_default()
            .push(bar);
    }

    for ((_year, _month), month_bars) in buckets {
        let start = month_bars.first().unwrap().datetime().date_naive();
        let end = month_bars.last().unwrap().datetime().date_naive();
        let path = out_dir.join(format!(
            "{}_{}-{}-{}_to_{}.csv",
            exchange,
            symbol,
            timeframe_label,
            start.format("%Y-%m-%d"),
            end.format("%Y-%m-%d")
        ));

        let mut f = fs::File::create(&path)?;
        writeln!(f, "time,open,high,low,close,volume")?;

        for bar in month_bars {
            writeln!(
                f,
                "{},{:.8},{:.8},{:.8},{:.8},{:.8}",
                bar.timestamp(),
                bar.open(),
                bar.high(),
                bar.low(),
                bar.close(),
                bar.volume()
            )?;
        }

        println!("wrote {}", path.display());
    }

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();
    let cfg = parse_args()?;

    let now = Utc::now();
    let default_from = (now - ChronoDuration::days(cfg.years_back * 365)).date_naive();
    let from_date = cfg.from_date.unwrap_or(default_from);
    let to_date = cfg.to_date.unwrap_or(now.date_naive());

    if from_date > to_date {
        return Err(anyhow!(
            "invalid range: --from {} is after --to {}",
            from_date,
            to_date
        ));
    }

    let target_ts = from_date
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| anyhow!("invalid --from date: {from_date}"))?
        .and_utc()
        .timestamp();

    let to_midnight_ts = to_date
        .succ_opt()
        .ok_or_else(|| anyhow!("--to date overflow: {to_date}"))?
        .and_hms_opt(0, 0, 0)
        .ok_or_else(|| anyhow!("invalid --to date: {to_date}"))?
        .and_utc()
        .timestamp();

    let range_end_ts = if cfg.to_date.is_some() {
        to_midnight_ts - cfg.step_seconds
    } else {
        now.timestamp()
    };

    let mut anchor = range_end_ts;

    if anchor < target_ts {
        return Err(anyhow!(
            "invalid range after alignment: anchor={} target={}",
            anchor,
            target_ts
        ));
    }

    println!(
        "Backfilling {}:{} {} from {} back to {}",
        cfg.exchange,
        cfg.symbol,
        cfg.timeframe_label,
        Utc.timestamp_opt(anchor, 0).single().unwrap().to_rfc3339(),
        Utc.timestamp_opt(target_ts, 0)
            .single()
            .unwrap()
            .to_rfc3339()
    );

    let auth_token = resolve_auth_token().await?;

    let mut rounds = 0_u32;
    let mut all = Vec::<DataPoint>::new();
    let mut seen = BTreeSet::<i64>::new();
    let started = Instant::now();

    while anchor >= target_ts {
        rounds += 1;
        let anchor_dt = Utc.timestamp_opt(anchor, 0).single().unwrap();
        println!("round={} replay_from={}", rounds, anchor_dt.to_rfc3339());

        let window = fetch_window(
            &auth_token,
            &cfg.exchange,
            &cfg.symbol,
            cfg.interval,
            anchor,
        )
        .await
        .with_context(|| format!("failed to fetch window for replay_from={anchor}"))?;

        if window.is_empty() {
            println!("window empty; stopping");
            break;
        }

        let oldest = window.first().unwrap().timestamp();
        let newest = window.last().unwrap().timestamp();

        let mut new_count = 0_usize;
        for bar in window {
            let ts = bar.timestamp();
            if ts < target_ts || ts > range_end_ts {
                continue;
            }
            if seen.insert(ts) {
                all.push(bar);
                new_count += 1;
            }
        }

        println!(
            "window={} -> {} | unique_added={} | total_unique={} | elapsed={}s",
            Utc.timestamp_opt(oldest, 0).single().unwrap().to_rfc3339(),
            Utc.timestamp_opt(newest, 0).single().unwrap().to_rfc3339(),
            new_count,
            seen.len(),
            started.elapsed().as_secs()
        );

        if oldest <= target_ts {
            println!("target reached");
            break;
        }

        let next_anchor = oldest - cfg.step_seconds;
        if next_anchor >= anchor {
            println!(
                "no backward progress (anchor={} oldest={}); stopping",
                anchor, oldest
            );
            break;
        }
        anchor = next_anchor;
    }

    all.sort_by_key(|d| d.timestamp());
    all.dedup_by_key(|d| d.timestamp());

    if all.is_empty() {
        return Err(anyhow!("no bars collected"));
    }

    let first = all.first().unwrap().datetime();
    let last = all.last().unwrap().datetime();
    println!(
        "final: {} bars, range {} -> {}",
        all.len(),
        first.to_rfc3339(),
        last.to_rfc3339()
    );

    write_monthly_csvs(
        &cfg.out_dir,
        &cfg.exchange,
        &cfg.symbol,
        &cfg.timeframe_label,
        &all,
    )?;

    Ok(())
}
