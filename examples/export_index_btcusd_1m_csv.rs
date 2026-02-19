use anyhow::{Context, Result, anyhow};
use chrono::{Datelike, TimeZone, Utc};
use std::{collections::BTreeMap, env, fs, io::Write, path::PathBuf, time::Duration};
use tradingview::{
    DataServer, Interval, OHLCV, UserCookies, client::misc::get_quote_token, history,
};

#[derive(Debug)]
struct Config {
    symbol: String,
    exchange: String,
    out_dir: PathBuf,
}

fn parse_args() -> Result<Config> {
    let mut symbol = "BTCUSD".to_string();
    let mut exchange = "INDEX".to_string();
    let mut out_dir = PathBuf::from("../../csv_exports");

    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--symbol" => {
                symbol = args
                    .next()
                    .ok_or_else(|| anyhow!("missing value for --symbol"))?;
            }
            "--exchange" => {
                exchange = args
                    .next()
                    .ok_or_else(|| anyhow!("missing value for --exchange"))?;
            }
            "--out-dir" => {
                out_dir = PathBuf::from(
                    args.next()
                        .ok_or_else(|| anyhow!("missing value for --out-dir"))?,
                );
            }
            "-h" | "--help" => {
                println!(
                    "Usage: cargo run --example export_index_btcusd_1m_csv -- \\\n  [--symbol BTCUSD] [--exchange INDEX] [--out-dir ../../csv_exports]"
                );
                std::process::exit(0);
            }
            other => return Err(anyhow!("unknown argument: {other}")),
        }
    }

    Ok(Config {
        symbol,
        exchange,
        out_dir,
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

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();
    let cfg = parse_args()?;

    println!(
        "Fetching {}:{} 1m bars with replay...",
        cfg.exchange, cfg.symbol
    );

    let auth_token = resolve_auth_token().await?;

    let (_info, mut data) = history::single::retrieve()
        .auth_token(&auth_token)
        .symbol(&cfg.symbol)
        .exchange(&cfg.exchange)
        .interval(Interval::OneMinute)
        .num_bars(2_000_000)
        .with_replay(true)
        .server(DataServer::ProData)
        .timeout_duration(Duration::from_secs(1200))
        .call()
        .await
        .context("historical replay fetch failed")?;

    if data.is_empty() {
        return Err(anyhow!("no data returned"));
    }

    data.sort_by_key(|d| d.timestamp());
    data.dedup_by_key(|d| d.timestamp());

    let first_ts = data.first().unwrap().timestamp();
    let last_ts = data.last().unwrap().timestamp();
    let first_dt = Utc.timestamp_opt(first_ts, 0).single().unwrap();
    let last_dt = Utc.timestamp_opt(last_ts, 0).single().unwrap();

    println!(
        "Retrieved {} bars ({} -> {})",
        data.len(),
        first_dt.to_rfc3339(),
        last_dt.to_rfc3339()
    );

    fs::create_dir_all(&cfg.out_dir)?;

    let mut buckets: BTreeMap<(i32, u32), Vec<_>> = BTreeMap::new();
    for bar in data {
        let dt = bar.datetime();
        buckets
            .entry((dt.year(), dt.month()))
            .or_default()
            .push(bar);
    }

    for ((_year, _month), bars) in buckets {
        let start = bars.first().unwrap().datetime().date_naive();
        let end = bars.last().unwrap().datetime().date_naive();
        let file_name = format!(
            "INDEX_{}-1m-{}_to_{}.csv",
            cfg.symbol,
            start.format("%Y-%m-%d"),
            end.format("%Y-%m-%d")
        );
        let path = cfg.out_dir.join(file_name);

        let mut f = fs::File::create(&path)?;
        writeln!(f, "time,open,high,low,close,volume")?;

        for bar in bars {
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

        println!("Wrote {}", path.display());
    }

    Ok(())
}
