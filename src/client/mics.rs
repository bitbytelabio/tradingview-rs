use std::sync::Arc;

use crate::{
    models::{
        pine_indicator::{self, BuiltinIndicators, PineInfo, PineMetadata, PineSearchResult},
        Symbol, SymbolSearch, UserCookies,
    },
    utils::build_request,
    Error, Result,
};
use reqwest::Response;
use tokio::{sync::Semaphore, task::JoinHandle};
use tracing::debug;

async fn get(client: Option<&UserCookies>, url: &str) -> Result<Response> {
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
pub async fn get_chart_token(client: &UserCookies, layout_id: &str) -> Result<String> {
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
        None => panic!("{:?}", "No token found"),
    }
}

#[tracing::instrument(skip(client))]
pub async fn get_drawing<T>(
    client: &UserCookies,
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
pub async fn get_private_indicators(client: &UserCookies) -> Result<Vec<PineInfo>> {
    let indicators = get(
        Some(client),
        "https://pine-facade.tradingview.com/pine-facade/list?filter=saved",
    )
    .await?
    .json::<Vec<PineInfo>>()
    .await?;
    Ok(indicators)
}

#[tracing::instrument]
pub async fn get_builtin_indicators(indicator_type: BuiltinIndicators) -> Result<Vec<PineInfo>> {
    let indicator_types = match indicator_type {
        BuiltinIndicators::All => vec!["fundamental", "standard", "candlestick"],
        BuiltinIndicators::Fundamental => vec!["fundamental"],
        BuiltinIndicators::Standard => vec!["standard"],
        BuiltinIndicators::Candlestick => vec!["candlestick"],
    };
    let mut indicators: Vec<PineInfo> = vec![];

    let mut tasks: Vec<JoinHandle<Result<Vec<PineInfo>>>> = Vec::new();

    for indicator_type in indicator_types {
        let url = format!(
            "https://pine-facade.tradingview.com/pine-facade/list/?filter={}",
            indicator_type
        );
        let task = tokio::spawn(async move {
            let data = get(None, &url).await?.json::<Vec<PineInfo>>().await?;
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
pub async fn search_indicator(
    client: Option<&UserCookies>,
    search: &str,
    offset: i32,
) -> Result<Vec<PineSearchResult>> {
    let url = format!(
        "https://www.tradingview.com/pubscripts-suggest-json/?search={}&offset={}",
        search, offset
    );
    let resp: pine_indicator::SearchResponse = get(client, &url).await?.json().await?;
    debug!("Response: {:?}", resp);

    if resp.result.is_empty() {
        return Err(Error::Generic("No results found".to_string()));
    }

    Ok(resp.result)
}

#[tracing::instrument(skip(client))]
pub async fn get_indicator_metadata(
    client: Option<&UserCookies>,
    pinescript_id: &str,
    pinescript_version: &str,
) -> Result<PineMetadata> {
    use urlencoding::encode;
    let url = format!(
        "https://pine-facade.tradingview.com/pine-facade/translate/{}/{}",
        encode(pinescript_id),
        encode(pinescript_version)
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
