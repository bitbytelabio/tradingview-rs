use std::{collections::HashMap, sync::Arc};

use crate::{
    models::{
        pine_indicator::{self, BuiltinIndicators, Info, Metadata},
        Screener, SimpleTA, Symbol, SymbolSearch,
    },
    prelude::*,
    user::User,
    utils::build_request,
};
use reqwest::Response;
use tokio::{sync::Semaphore, task::JoinHandle};
use tracing::{debug, warn};

const INDICATORS: [&str; 3] = ["Recommend.Other", "Recommend.All", "Recommend.MA"];

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

async fn get(client: Option<&User>, url: &str) -> Result<Response> {
    if let Some(client) = client {
        let cookie = format!(
            "sessionid={}; sessionid_sign={};",
            client.session, client.signature
        );
        let client = build_request(Some(&cookie))?;
        let response = client.get(url).send().await?;
        return Ok(response);
    }
    Ok(build_request(None)?.get(url).send().await?)
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
            screener = screener
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
        Some(data) => Ok(data.clone()),
        None => Err(Error::NoScanDataFound),
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
        .flat_map(|t| {
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
        .collect();

    match fetch_scan_data(client, symbols, screener, cols.clone()).await {
        Ok(data) => {
            let mut advices: Vec<SimpleTA> = vec![];

            data.as_array().unwrap_or(&vec![]).iter().for_each(|s| {
                let mut advice = SimpleTA {
                    name: s["s"].as_str().unwrap_or("").to_string(),
                    ..Default::default()
                };
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
            Error::NoScanDataFound => Err(Error::SymbolsNotInSameExchange),
            _ => Err(e),
        },
    }
}

#[tracing::instrument]
pub async fn search_symbol(
    search: &str,
    exchange: &str,
    search_type: &str,
    start: u64,
    country: &str,
) -> Result<SymbolSearch> {
    let search_data: SymbolSearch = get(None, &format!(
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

#[tracing::instrument]
pub async fn list_symbols(market_type: Option<String>) -> Result<Vec<Symbol>> {
    let search_type = Arc::new(market_type.unwrap_or_default());

    let search_symbol_reps = search_symbol("", "", &search_type, 0, "").await?;
    let remaining = search_symbol_reps.remaining;
    let mut symbols = search_symbol_reps.symbols;

    let max_concurrent_tasks = 50;
    let semaphore = Arc::new(Semaphore::new(max_concurrent_tasks));

    let mut tasks = Vec::new();

    for i in (50..remaining).step_by(50) {
        let search_type = Arc::clone(&search_type);
        let semaphore = Arc::clone(&semaphore);

        let task = tokio::spawn(async move {
            let _permit = semaphore.acquire().await.unwrap();
            search_symbol("", "", &search_type, i, "")
                .await
                .map(|resp| resp.symbols)
        });

        tasks.push(task);
    }

    for handler in tasks {
        symbols.extend(handler.await??);
    }

    Ok(symbols)
}

#[tracing::instrument(skip(client))]
pub async fn get_chart_token(client: &User, layout_id: &str) -> Result<String> {
    let data: serde_json::Value = get(
        Some(client),
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

    Ok(get(Some(client), &url).await?.json().await?)
}

#[tracing::instrument(skip(client))]
pub async fn get_private_indicators(client: &User) -> Result<Vec<Info>> {
    let indicators = get(
        Some(client),
        "https://pine-facade.tradingview.com/pine-facade/list?filter=saved",
    )
    .await?
    .json::<Vec<Info>>()
    .await?;
    Ok(indicators)
}

#[tracing::instrument]
pub async fn get_builtin_indicators(indicator_type: BuiltinIndicators) -> Result<Vec<Info>> {
    let indicator_types = match indicator_type {
        BuiltinIndicators::All => vec!["fundamental", "standard", "candlestick"],
        BuiltinIndicators::Fundamental => vec!["fundamental"],
        BuiltinIndicators::Standard => vec!["standard"],
        BuiltinIndicators::Candlestick => vec!["candlestick"],
    };
    let mut indicators: Vec<Info> = vec![];

    let mut tasks: Vec<JoinHandle<Result<Vec<Info>>>> = Vec::new();

    for indicator_type in indicator_types {
        let url = format!(
            "https://pine-facade.tradingview.com/pine-facade/list/?filter={}",
            indicator_type
        );
        let task = tokio::spawn(async move {
            let data = get(None, &url).await?.json::<Vec<Info>>().await?;
            Ok(data)
        });

        tasks.push(task);
    }

    for handler in tasks {
        indicators.extend(handler.await??);
    }

    Ok(indicators)
}

#[tracing::instrument(skip(client))]
pub async fn get_indicator_metadata(client: Option<&User>, indicator: &Info) -> Result<Metadata> {
    use urlencoding::encode;
    let url = format!(
        "https://pine-facade.tradingview.com/pine-facade/translate/{}/{}",
        encode(&indicator.script_id_part),
        encode(&indicator.version)
    );
    debug!("URL: {}", url);
    let resp: pine_indicator::TranslateResponse = get(client, &url).await?.json().await?;

    if resp.success {
        return Ok(resp.result);
    }

    Err(Error::Generic(
        "Failed to get indicator metadata".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use crate::client::mics::*;

    #[test]
    fn test_get_screener() {
        let exchanges = vec![
            ("NASDAQ", Ok(Screener::America)),
            ("ASX", Ok(Screener::Australia)),
            ("TSX", Ok(Screener::Canada)),
            ("EGX", Ok(Screener::Egypt)),
            ("FWB", Ok(Screener::Germany)),
            ("BSE", Ok(Screener::India)),
            ("TASE", Ok(Screener::Israel)),
            ("MIL", Ok(Screener::Italy)),
            ("LUXSE", Ok(Screener::Luxembourg)),
            ("NEWCONNECT", Ok(Screener::Poland)),
            ("NGM", Ok(Screener::Sweden)),
            ("BIST", Ok(Screener::Turkey)),
            ("LSE", Ok(Screener::UK)),
            ("HNX", Ok(Screener::Vietnam)),
            ("invalid", Err(Error::InvalidExchange)),
        ];

        for (exchange, expected) in exchanges {
            let result = get_screener(exchange);
            match (result, expected) {
                (Ok(actual), Ok(expected)) => {
                    assert_eq!(actual, expected, "Exchange: {}", exchange)
                }
                (Err(actual), Err(expected)) => assert_eq!(
                    actual.to_string(),
                    expected.to_string(),
                    "Exchange: {}",
                    exchange
                ),
                _ => panic!("Unexpected result for exchange: {}", exchange),
            }
        }
    }
}
