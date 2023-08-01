use std::{collections::HashMap, sync::Arc};

use crate::{
    model::{IndicatorInfo, IndicatorMetadata, SimpleTA, Symbol, SymbolSearch},
    prelude::*,
    user::User,
};
use reqwest::Response;
use tokio::{sync::Semaphore, task::JoinHandle};
use tracing::{debug, warn};

const INDICATORS: [&str; 3] = ["Recommend.Other", "Recommend.All", "Recommend.MA"];

#[derive(Debug, PartialEq)]
pub enum Screener {
    America,
    Australia,
    Canada,
    Egypt,
    Germany,
    India,
    Israel,
    Italy,
    Luxembourg,
    Poland,
    Sweden,
    Turkey,
    UK,
    Vietnam,
    Other(String),
}

impl std::fmt::Display for Screener {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Screener::America => write!(f, "america"),
            Screener::Australia => write!(f, "australia"),
            Screener::Canada => write!(f, "canada"),
            Screener::Egypt => write!(f, "egypt"),
            Screener::Germany => write!(f, "germany"),
            Screener::India => write!(f, "india"),
            Screener::Israel => write!(f, "israel"),
            Screener::Italy => write!(f, "italy"),
            Screener::Luxembourg => write!(f, "luxembourg"),
            Screener::Poland => write!(f, "poland"),
            Screener::Sweden => write!(f, "sweden"),
            Screener::Turkey => write!(f, "turkey"),
            Screener::UK => write!(f, "uk"),
            Screener::Vietnam => write!(f, "vietnam"),
            Screener::Other(s) => write!(f, "{}", s.to_lowercase()),
        }
    }
}

#[tracing::instrument]
fn get_screener(exchange: &str) -> Result<Screener> {
    let e = exchange.to_uppercase();
    let screener = match e.as_str() {
        "NASDAQ" | "NYSE" | "NYSE ARCA" | "OTC" => Screener::America,
        "ASX" => Screener::Australia,
        "TSX" | "TSXV" | "CSE" | "NEO" => Screener::Canada,
        "EGX" => Screener::Egypt,
        "FWB" | "SWB" | "XETR" => Screener::Germany,
        "BSE" | "NSE" => Screener::India,
        "TASE" => Screener::Israel,
        "MIL" | "MILSEDEX" => Screener::Italy,
        "LUXSE" => Screener::Luxembourg,
        "NEWCONNECT" => Screener::Poland,
        "NGM" => Screener::Sweden,
        "BIST" => Screener::Turkey,
        "LSE" | "LSIN" => Screener::UK,
        "HNX" | "HOSE" | "UPCOM" => Screener::Vietnam,
        _ => return Err(Error::InvalidExchange),
    };
    Ok(screener)
}

async fn get(client: &User, url: &str) -> Result<Response> {
    let cookie = format!(
        "sessionid={}; sessionid_sign={};",
        client.session, client.signature
    );
    let client: reqwest::Client = crate::utils::build_request(Some(&cookie))?;
    let response = client.get(url).send().await?;
    Ok(response)
}

async fn post<T: serde::Serialize>(client: &User, url: &str, body: T) -> Result<Response> {
    let cookie = format!(
        "sessionid={}; sessionid_sign={};",
        client.session, client.signature
    );
    let client: reqwest::Client = crate::utils::build_request(Some(&cookie))?;
    let response = client.post(url).json(&body).send().await?;
    Ok(response)
}

#[tracing::instrument(skip(client))]
async fn fetch_scan_data(
    client: &User,
    tickers: Vec<String>,
    screener: Screener,
    columns: Vec<String>,
) -> Result<serde_json::Value> {
    let resp_body: serde_json::Value = post(
        client,
        &format!(
            "https://scanner.tradingview.com/{screener}/scan",
            screener = screener.to_string()
        ),
        serde_json::json!({
            "symbols": {
                "tickers": tickers
            },
            "columns": columns
        }),
    )
    .await?
    .json()
    .await?;

    let data = resp_body.get("data");
    match data {
        Some(data) => {
            return Ok(data.clone());
        }
        None => return Err(Error::NoScanDataFound),
    }
}

#[tracing::instrument(skip(client))]
pub async fn get_ta(client: &User, exchange: &str, symbols: &[&str]) -> Result<Vec<SimpleTA>> {
    if exchange.is_empty() {
        return Err(Error::ExchangeNotSpecified);
    } else if symbols.is_empty() {
        return Err(Error::SymbolsNotSpecified);
    }
    let symbols = symbols
        .iter()
        .map(|s| format!("{}:{}", exchange, s))
        .collect::<Vec<String>>();

    let screener = get_screener(exchange)?;
    let cols: Vec<String> = vec!["1", "5", "15", "60", "240", "1D", "1W", "1M"]
        .iter()
        .map(|t| {
            INDICATORS
                .iter()
                .map(|i| {
                    if t != &"1D" {
                        format!("{}|{}", i, t)
                    } else {
                        i.to_string()
                    }
                })
                .collect::<Vec<String>>()
        })
        .flatten()
        .collect();

    match fetch_scan_data(client, symbols, screener, cols.clone()).await {
        Ok(data) => {
            let mut advices: Vec<SimpleTA> = vec![];

            data.as_array().unwrap_or(&vec![]).iter().for_each(|s| {
                let mut advice = SimpleTA::default();
                advice.name = s["s"].as_str().unwrap_or("").to_string();
                s["d"]
                    .as_array()
                    .unwrap_or(&vec![])
                    .iter()
                    .enumerate()
                    .for_each(|(i, val)| {
                        let (name, period) =
                            cols[i].split_once('|').unwrap_or((cols[i].as_str(), ""));
                        let p_name = if period.is_empty() { "1D" } else { period };
                        let name = name.split('.').last().unwrap_or(name);
                        let val = (val.as_f64().unwrap_or(0.0) * 1000.0 / 500.0).round() / 1000.0;
                        advice
                            .data
                            .entry(p_name.to_string())
                            .or_insert_with(HashMap::new)
                            .insert(name.to_string(), val);
                    });
                advices.push(advice);
            });
            println!("{:#?}", advices);
            Ok(advices)
        }
        Err(e) => match e {
            Error::NoScanDataFound => {
                return Err(Error::SymbolsNotInSameExchange);
            }
            _ => {
                return Err(e);
            }
        },
    }
}

#[tracing::instrument(skip(client))]
pub async fn search_symbol(
    client: &User,
    search: &str,
    exchange: &str,
    search_type: &str,
    start: u64,
    country: &str,
) -> Result<SymbolSearch> {
    let search_data: SymbolSearch = get(client, &format!(
                "https://symbol-search.tradingview.com/symbol_search/v3/?text={search}&hl=1&exchange={exchange}&lang=en&search_type={search_type}&start={start}&domain=production&sort_by_country={country}",
                search = search,
                exchange = exchange,
                search_type = search_type,
                start = start,
                country = country
            ))
            .await?
            .json()
            .await?;
    Ok(search_data)
}

#[tracing::instrument(skip(client))]
pub async fn list_symbols(client: &User, market_type: Option<String>) -> Result<Vec<Symbol>> {
    let search_type = Arc::new(market_type.unwrap_or("".to_string()));
    let user = Arc::new(client.clone());

    let search_symbol_reps = search_symbol(&user, "", "", &search_type, 0, "").await?;
    let remaining = search_symbol_reps.remaining;
    let mut symbols = search_symbol_reps.symbols;

    // Set the maximum number of tasks running concurrently here (e.g., 10).
    let max_concurrent_tasks = 10;
    let semaphore = Arc::new(Semaphore::new(max_concurrent_tasks));

    let mut tasks: Vec<JoinHandle<Result<Vec<Symbol>>>> = vec![];

    for i in (50..remaining).step_by(50) {
        let user_clone = Arc::clone(&user);
        let search_type_clone = Arc::clone(&search_type);
        let semaphore_clone = Arc::clone(&semaphore);

        let task = tokio::spawn(async move {
            // Acquire the semaphore permit before running the task.
            let _permit = semaphore_clone.acquire().await.unwrap();
            let resp = search_symbol(&user_clone, "", "", &search_type_clone, i, "").await?;
            Ok(resp.symbols)
        });

        tasks.push(task);
    }

    for handler in tasks {
        let resp_symbols = handler.await??;
        symbols.extend(resp_symbols);
    }

    Ok(symbols)
}

#[tracing::instrument(skip(client))]
pub async fn get_chart_token(client: &User, layout_id: &str) -> Result<String> {
    let data: serde_json::Value = get(
        client,
        &format!(
            "https://www.tradingview.com/chart-token/?image_url={}&user_id={}",
            layout_id, client.id
        ),
    )
    .await?
    .json()
    .await?;

    match data.get("token") {
        Some(token) => {
            return Ok(match token.as_str() {
                Some(token) => token.to_string(),
                None => return Err(Error::NoChartTokenFound),
            })
        }
        None => Err("No token found").unwrap(),
    }
}

#[tracing::instrument(skip(client))]
pub async fn get_drawing<T>(
    client: &User,
    layout_id: &str,
    symbol: &str,
    chart_id: T,
) -> Result<serde_json::Value>
where
    T: std::fmt::Display + std::fmt::Debug,
{
    let token = get_chart_token(client, layout_id).await?;
    debug!("Chart token: {}", token);
    let url = format!(
            "https://charts-storage.tradingview.com/charts-storage/get/layout/{layout_id}/sources?chart_id={chart_id}&jwt={token}&symbol={symbol}",
            layout_id = layout_id,
            chart_id = chart_id,
            token = token,
            symbol = symbol
        );

    Ok(get(client, &url).await?.json().await?)
}

#[tracing::instrument(skip(client))]
pub async fn get_private_indicators(client: &User) -> Result<Vec<IndicatorInfo>> {
    let indicators = get(
        client,
        "https://pine-facade.tradingview.com/pine-facade/list?filter=saved",
    )
    .await?
    .json::<Vec<IndicatorInfo>>()
    .await?;
    Ok(indicators)
}

#[tracing::instrument(skip(client))]
pub async fn get_builtin_indicators(client: &User) -> Result<Vec<IndicatorInfo>> {
    let indicator_types = vec!["standard", "candlestick", "fundamental"];
    let mut indicators: Vec<IndicatorInfo> = vec![];
    for indicator_type in indicator_types {
        let url = format!(
            "https://pine-facade.tradingview.com/pine-facade/list/?filter={}",
            indicator_type
        );

        let mut data = get(client, &url)
            .await?
            .json::<Vec<IndicatorInfo>>()
            .await?;

        indicators.append(&mut data);
    }
    Ok(indicators)
}

#[tracing::instrument(skip(client))]
pub async fn get_indicator_metadata(
    client: &User,
    indicator: &IndicatorInfo,
) -> Result<IndicatorMetadata> {
    use urlencoding::encode;
    let url = format!(
        "https://pine-facade.tradingview.com/pine-facade/translate/{}/{}",
        encode(&indicator.id),
        encode(&indicator.version)
    );
    debug!("URL: {}", url);
    let data: serde_json::Value = get(client, &url).await?.json().await?;

    if !data["success"].as_bool().unwrap_or(false)
        || !data["result"]["metaInfo"]["inputs"].is_array()
    {
        return Err(Error::IndicatorDataNotFound(
            data["reason"].as_str().unwrap_or_default().to_string(),
        ));
    }

    let result: IndicatorMetadata = serde_json::from_value(data["result"].clone())?;

    Ok(result)
}
