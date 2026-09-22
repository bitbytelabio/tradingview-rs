pub use crate::models::{
    Period, PeriodRecommendation, TechnicalAnalysis, TechnicalAnalysisPeriod,
    TechnicalAnalysisRecommendation, TechnicalAnalysisRecommendations,
};
use crate::{
    ChartDrawing, Country, CryptoCentralization, EconomicCategory, EconomicSource,
    FuturesProductType, MarketType, Result, StockSector, Symbol, SymbolSearchResponse, UserCookies,
    error::Error,
    pine_indicator::{self, BuiltinIndicators, PineInfo, PineMetadata, PineSearchResult},
    utils::http_client,
};
use bon::builder;
use reqwest::{Response, header::COOKIE};
use serde_json::Value;
use std::sync::Arc;
use tokio::{sync::Semaphore, task::JoinHandle};
use tracing::debug;
use urlencoding::encode;
use ustr::Ustr;

static SEARCH_BASE_URL: &str = "https://symbol-search.tradingview.com/symbol_search/v3/";
static DEFAULT_LANGUAGE: &str = "en";
static DEFAULT_HIGHLIGHT: &str = "0";

#[derive(Debug, Clone)]
struct ParameterBuilder {
    params: Vec<(String, String)>,
}

impl ParameterBuilder {
    fn new() -> Self {
        Self { params: Vec::new() }
    }

    fn add<T: ToString>(&mut self, key: &str, value: T) -> &mut Self {
        self.params.push((key.to_string(), value.to_string()));
        self
    }

    fn add_optional<T: ToString>(&mut self, key: &str, value: Option<T>) -> &mut Self {
        if let Some(v) = value {
            self.add(key, v);
        }
        self
    }

    fn add_if_not_empty(&mut self, key: &str, value: &str) -> &mut Self {
        if !value.is_empty() {
            self.add(key, value);
        }
        self
    }

    fn build(&self) -> String {
        self.params
            .iter()
            .map(|(k, v)| format!("{}={}", encode(k), encode(v)))
            .collect::<Vec<String>>()
            .join("&")
    }
}

/// Sends an HTTP GET request using the shared HTTP client for connection pooling.
///
/// # Arguments
///
/// * `client` - An optional reference to a `UserCookies` struct representing the client to use for the request.
/// * `url` - A string slice representing the URL to send the request to.
///
/// # Returns
///
/// A `Result` containing a `Response` struct representing the response from the server, or an error if the request failed.
async fn get(client: Option<&UserCookies>, url: &str) -> Result<Response> {
    let mut req = http_client().get(url);
    if let Some(c) = client {
        let cookie = format!(
            "sessionid={}; sessionid_sign={}; device_t={};",
            c.session, c.session_signature, c.device_token
        );
        req = req.header(COOKIE, &cookie);
    }
    Ok(req.send().await?)
}

pub async fn get_symbol(symbol: &str, exchange: &str) -> Option<Symbol> {
    let search_data = advanced_search_symbol()
        .search(symbol)
        .exchange(exchange)
        .call()
        .await
        .ok()?;

    let symbol = match search_data.symbols.first() {
        Some(symbol) => symbol,
        None => {
            return None;
        }
    };

    Some(symbol.to_owned())
}

pub async fn search_symbols(search: &str, exchange: &str) -> Result<Vec<Symbol>> {
    let search_data = advanced_search_symbol()
        .search(search)
        .exchange(exchange)
        .call()
        .await?;
    Ok(search_data.symbols)
}

/// Searches for a symbol using the specified search parameters.
///
/// # Arguments
///
/// * `search` - A string slice representing the search query.
/// * `exchange` - A string slice representing the exchange to search in.
/// * `market_type` - A `SymbolMarketType` enum representing the type of market to search in.
/// * `start` - An unsigned 64-bit integer representing the starting index of the search results.
/// * `country` - A string slice representing the country to search in.
/// * `domain` - A string slice representing the domain to search in. Defaults to "production" if empty.
///
/// # Returns
///
/// A `Result` containing a `SymbolSearchResponse` struct representing the search results, or an error if the search failed.
#[builder]
pub async fn advanced_search_symbol(
    search: Option<&str>,
    exchange: Option<&str>,
    #[builder(default = MarketType::All)] market_type: MarketType,
    #[builder(default = 0)] start: u64,
    country: Option<Country>,
    #[builder(default = "production")] domain: &str,
    futures_type: Option<FuturesProductType>, // For Futures Only
    stock_sector: Option<StockSector>,        // For Stock Only
    crypto_centralization: Option<CryptoCentralization>, // For Crypto Only
    economic_source: Option<EconomicSource>,  // For Economy Only
    economic_category: Option<EconomicCategory>, // For Economy Only
    search_type: Option<&str>, // For Advanced Search Only, disabled market_type if is Some
) -> Result<SymbolSearchResponse> {
    let mut builder = ParameterBuilder::new();

    // Add basic parameters
    builder
        .add("text", search.unwrap_or_default())
        .add("exchange", exchange.unwrap_or_default())
        .add("domain", domain)
        .add("hl", DEFAULT_HIGHLIGHT)
        .add("lang", DEFAULT_LANGUAGE)
        .add("start", start);

    // Handle search_type vs market_type logic
    if let Some(search_type) = search_type {
        builder.add_if_not_empty("search_type", search_type);
    } else {
        builder.add("search_type", market_type.to_string());
    }

    // Add country parameters
    if let Some(country) = country {
        let country_str = country.to_string();
        builder
            .add("country", &country_str)
            .add("sort_by_country", &country_str);
    }

    // Add market-specific parameters
    add_market_specific_params(
        &mut builder,
        market_type,
        futures_type,
        stock_sector,
        crypto_centralization,
        economic_source,
        economic_category,
    );

    let params_str = builder.build();
    let url = format!("{SEARCH_BASE_URL}?{params_str}");

    // Validate URL length (most browsers/servers have ~8KB limit)
    if url.len() > 8000 {
        return Err(Error::Internal(Ustr::from(
            "URL too long - please reduce search parameters",
        )));
    }

    let search_data: SymbolSearchResponse = get(None, &url)
        .await
        .map_err(|e| Error::Internal(Ustr::from(&format!("Failed to fetch symbol search: {e}"))))?
        .json()
        .await
        .map_err(|e| {
            Error::Internal(Ustr::from(&format!(
                "Failed to parse symbol search response: {e}"
            )))
        })?;

    Ok(search_data)
}

fn add_market_specific_params(
    builder: &mut ParameterBuilder,
    market_type: MarketType,
    futures_type: Option<FuturesProductType>,
    stock_sector: Option<StockSector>,
    crypto_centralization: Option<CryptoCentralization>,
    economic_source: Option<EconomicSource>,
    economic_category: Option<EconomicCategory>,
) {
    match market_type {
        MarketType::Futures => {
            builder.add_optional("product", futures_type);
        }
        MarketType::Stocks(_) => {
            builder.add_optional("sector", stock_sector);
        }
        MarketType::Crypto(_) => {
            builder.add_optional("centralization", crypto_centralization);
        }
        MarketType::Economy => {
            builder
                .add_optional("source_id", economic_source)
                .add_optional("economic_category", economic_category);
        }
        _ => {}
    }
}

/// Lists symbols based on the specified search parameters.
///
/// # Arguments
///
/// * `exchange` - An optional string representing the exchange to search in.
/// * `market_type` - An optional `SymbolMarketType` enum representing the type of market to search in.
/// * `country` - An optional string representing the country to search in.
/// * `domain` - An optional string representing the domain to search in.
///
/// # Returns
///
/// A `Result` containing a vector of `Symbol` structs representing the search results, or an error if the search failed.
#[builder]
pub async fn list_symbols(
    exchange: Option<&str>,
    #[builder(default = MarketType::All)] market_type: MarketType,
    country: Option<Country>,
    search_type: Option<&str>, // For Advanced Search Only, disabled market_type if is Some
    domain: Option<&str>,
) -> Result<Vec<Symbol>> {
    let exchange = exchange.unwrap_or("").to_string();
    let domain = domain.unwrap_or("production").to_string();
    let search_type = search_type.unwrap_or("").to_string();

    // Get initial batch of symbols
    let search_symbol_reps = advanced_search_symbol()
        .exchange(&exchange)
        .market_type(market_type)
        .maybe_country(country)
        .search_type(&search_type)
        .domain(&domain)
        .call()
        .await?;

    let remaining = search_symbol_reps.remaining;
    let mut symbols = search_symbol_reps.symbols;

    // Early return if we already have all symbols
    if remaining <= 50 {
        return Ok(symbols);
    }

    let max_concurrent_tasks = 30;
    let semaphore = Arc::new(Semaphore::new(max_concurrent_tasks));
    let mut tasks = Vec::new();

    // Create tasks for remaining batches
    for i in (50..remaining).step_by(50) {
        let exchange = exchange.clone();
        let domain = domain.clone();
        let search_type = search_type.clone();
        let semaphore = Arc::clone(&semaphore);

        let task = tokio::spawn(async move {
            let _permit = semaphore.acquire().await.map_err(|e| {
                Error::Internal(Ustr::from(&format!("Failed to acquire semaphore: {e}")))
            })?;

            advanced_search_symbol()
                .exchange(&exchange)
                .maybe_country(country)
                .market_type(market_type)
                .search_type(&search_type)
                .domain(&domain)
                .start(i)
                .call()
                .await
                .map(|resp| resp.symbols)
                .map_err(|e| {
                    Error::Internal(Ustr::from(&format!(
                        "Failed to fetch symbols for batch starting at {i}: {e}"
                    )))
                })
        });

        tasks.push(task);
    }

    // Collect results from all tasks
    for (index, task) in tasks.into_iter().enumerate() {
        match task.await {
            Ok(Ok(batch_symbols)) => symbols.extend(batch_symbols),
            Ok(Err(e)) => return Err(e),
            Err(join_err) => {
                return Err(Error::Internal(Ustr::from(&format!(
                    "Task join failed for batch {index}: {join_err}"
                ))));
            }
        }
    }

    Ok(symbols)
}

/// Retrieves a chart token for the specified layout ID using the provided client.
///
/// # Arguments
///
/// * `client` - A reference to a `UserCookies` struct representing the client to use for the request.
/// * `layout_id` - A string slice representing the layout ID to retrieve the chart token for.
///
/// # Returns
///
/// A `Result` containing a string representing the chart token, or an error if the token could not be retrieved.
#[tracing::instrument(skip(client))]
pub async fn get_chart_token(client: &UserCookies, layout_id: &str) -> Result<String> {
    let data: Value = get(
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
        Some(token) => Ok(match token.as_str() {
            Some(token) => token.to_string(),
            None => {
                return Err(Error::NoChartTokenFound);
            }
        }),
        None => Err(Error::NoChartTokenFound),
    }
}

/// Retrieves the quote token from TradingView.
///
/// # Arguments
///
/// * `client` - A reference to a `UserCookies` struct containing the user's cookies.
///
/// # Returns
///
/// A `Result` containing a `String` with the quote token if successful, or an error if the request fails.
#[tracing::instrument(skip(client))]
pub async fn get_quote_token(client: &UserCookies) -> Result<String> {
    let data: String = get(Some(client), "https://www.tradingview.com/quote_token")
        .await?
        .json()
        .await?;
    Ok(data)
}

/// Retrieves a chart drawing from TradingView's charts-storage API.
///
/// # Arguments
///
/// * `client` - A reference to a `UserCookies` instance.
/// * `layout_id` - The ID of the chart layout.
/// * `symbol` - The symbol of the financial instrument to retrieve the chart drawing for.
/// * `chart_id` - (Optional) The ID of the chart to retrieve. If not provided, the shared chart will be retrieved.
///
/// # Returns
///
/// A `Result` containing a `ChartDrawing` instance if successful, or an error if the request fails.
#[tracing::instrument(skip(client))]
pub async fn get_drawing(
    client: &UserCookies,
    layout_id: &str,
    symbol: &str,
    chart_id: Option<&str>,
) -> Result<ChartDrawing> {
    let token = get_chart_token(client, layout_id).await?;

    debug!("Chart token: {}", token);
    let url = format!(
        "https://charts-storage.tradingview.com/charts-storage/get/layout/{layout_id}/sources?chart_id={chart_id}&jwt={token}&symbol={symbol}",
        chart_id = chart_id.unwrap_or("_shared"),
    );

    let response_data: ChartDrawing = get(Some(client), &url).await?.json().await?;

    Ok(response_data)
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

// Retrieves a list of built-in indicators of the specified type.
///
/// # Arguments
///
/// * `indicator_type` - A `BuiltinIndicators` enum representing the type of built-in indicators to retrieve.
///
/// # Returns
///
/// A `Result` containing a vector of `PineInfo` structs representing the built-in indicators, or an error if the indicators could not be retrieved.
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
            "https://pine-facade.tradingview.com/pine-facade/list/?filter={indicator_type}"
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

/// Searches for indicators on TradingView.
///
/// # Arguments
///
/// * `client` - An optional reference to a `UserCookies` object.
/// * `search` - A string slice containing the search query.
/// * `offset` - An integer representing the offset of the search results.
///
/// # Returns
///
/// A `Result` containing a vector of `PineSearchResult` objects if successful, or an `Error` if unsuccessful.
///
/// # Example
///
/// ```rust
/// use tradingview::search_indicator;
///
/// #[tokio::main]
/// async fn main() {
///     let results = search_indicator(None, "rsi", 0).await.unwrap();
///     println!("{:?}", results);
/// }
/// ```
#[tracing::instrument(skip(client))]
pub async fn search_indicator(
    client: Option<&UserCookies>,
    search: &str,
    offset: i32,
) -> Result<Vec<PineSearchResult>> {
    let url = format!(
        "https://www.tradingview.com/pubscripts-suggest-json/?search={search}&offset={offset}",
    );
    let resp: pine_indicator::SearchResponse = get(client, &url).await?.json().await?;
    debug!("Response: {:?}", resp);

    if resp.results.is_empty() {
        return Err(Error::Internal(Ustr::from(
            "No indicators found for the given search query",
        )));
    }

    Ok(resp.results)
}

/// Retrieves metadata for a TradingView Pine indicator.
///
/// # Arguments
///
/// * `client` - An optional reference to a `UserCookies` struct.
/// * `pinescript_id` - The ID of the Pine script.
/// * `pinescript_version` - The version of the Pine script.
///
/// # Returns
///
/// Returns a `Result` containing a `PineMetadata` struct if successful, or an `Error` if unsuccessful.
///
/// # Examples
///
/// ```rust
/// use tradingview::get_indicator_metadata;
///
/// async fn run() -> Result<(), Box<dyn std::error::Error>> {
///     let client = None;
///     let pinescript_id = "PUB;2187";
///     let pinescript_version = "-1";
///
///     let metadata = get_indicator_metadata(client.as_ref(), pinescript_id, pinescript_version).await?;
///     println!("{:?}", metadata);
///     Ok(())
/// }
/// ```
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

    Err(Error::Internal(Ustr::from(&format!(
        "Failed to retrieve metadata for Pine script ID: {pinescript_id}, Version: {pinescript_version}"
    ))))
}

pub(crate) const TA_SCAN_URL: &str = "https://scanner.tradingview.com/global/scan";

pub(crate) const TA_COLUMNS: [&str; 24] = [
    "Recommend.Other|1",
    "Recommend.All|1",
    "Recommend.MA|1",
    "Recommend.Other|5",
    "Recommend.All|5",
    "Recommend.MA|5",
    "Recommend.Other|15",
    "Recommend.All|15",
    "Recommend.MA|15",
    "Recommend.Other|60",
    "Recommend.All|60",
    "Recommend.MA|60",
    "Recommend.Other|240",
    "Recommend.All|240",
    "Recommend.MA|240",
    "Recommend.Other",
    "Recommend.All",
    "Recommend.MA",
    "Recommend.Other|1W",
    "Recommend.All|1W",
    "Recommend.MA|1W",
    "Recommend.Other|1M",
    "Recommend.All|1M",
    "Recommend.MA|1M",
];

#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
pub(crate) struct ScanSymbols<'a> {
    pub tickers: Vec<&'a str>,
}

#[derive(Debug, Clone, serde::Serialize, PartialEq, Eq)]
pub(crate) struct ScanRequest<'a> {
    pub symbols: ScanSymbols<'a>,
    pub columns: &'static [&'static str],
}

#[derive(Debug, Clone, serde::Deserialize)]
pub(crate) struct ScanResponseRow {
    #[allow(dead_code)]
    #[serde(default)]
    pub s: String,
    pub d: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub(crate) struct ScanResponse {
    #[allow(dead_code)]
    #[serde(default)]
    pub total_count: Option<u64>,
    #[serde(default)]
    pub data: Vec<ScanResponseRow>,
}

#[inline]
pub(crate) fn normalize_recommendation(val: f64) -> f64 {
    let rounded = (val * 1000.0).round() / 500.0;
    if rounded == 0.0 { 0.0 } else { rounded }
}

pub(crate) fn parse_technical_analysis_scan_response(
    scan_resp: &ScanResponse,
) -> Result<TechnicalAnalysis> {
    if scan_resp.data.is_empty() {
        return Err(Error::NoScanDataFound);
    }

    let row = &scan_resp.data[0];
    if row.d.len() < 24 {
        return Err(Error::JsonParse(Ustr::from(&format!(
            "Insufficient columns in scan data: expected at least 24, got {}",
            row.d.len()
        ))));
    }

    let mut values = [0.0f64; 24];
    for (i, val_json) in row.d[..24].iter().enumerate() {
        let num = match val_json {
            serde_json::Value::Number(n) => n.as_f64().ok_or_else(|| {
                Error::JsonParse(Ustr::from(&format!(
                    "Invalid number format at column index {i}"
                )))
            })?,
            serde_json::Value::Null => {
                return Err(Error::JsonParse(Ustr::from(&format!(
                    "Null value at column index {i}"
                ))));
            }
            _ => {
                return Err(Error::JsonParse(Ustr::from(&format!(
                    "Non-numeric value at column index {i}"
                ))));
            }
        };

        if !num.is_finite() {
            return Err(Error::JsonParse(Ustr::from(&format!(
                "Non-finite number at column index {i}"
            ))));
        }

        values[i] = normalize_recommendation(num);
    }

    let make_rec = |offset: usize| TechnicalAnalysisRecommendations {
        other: values[offset],
        all: values[offset + 1],
        ma: values[offset + 2],
    };

    Ok(TechnicalAnalysis {
        period_1m: make_rec(0),
        period_5m: make_rec(3),
        period_15m: make_rec(6),
        period_1h: make_rec(9),
        period_4h: make_rec(12),
        period_1d: make_rec(15),
        period_1w: make_rec(18),
        period_1m_month: make_rec(21),
    })
}

pub(crate) fn parse_technical_analysis_response(body: &str) -> Result<TechnicalAnalysis> {
    let scan_resp: ScanResponse = serde_json::from_str(body)?;
    parse_technical_analysis_scan_response(&scan_resp)
}

/// Retrieves technical analysis recommendations for the specified symbol.
///
/// Sends a request to TradingView's global scanner (`https://scanner.tradingview.com/global/scan`)
/// requesting recommendations across 8 standard timeframes (1m, 5m, 15m, 1h, 4h, 1d, 1w, 1M)
/// for oscillators (`Other`), moving averages (`MA`), and summary (`All`).
///
/// # Arguments
///
/// * `symbol` - Symbol identifier, e.g. `"AMEX:SPY"` or `"BINANCE:BTCUSDT"`.
///
/// # Errors
///
/// Returns [`Error::NoScanDataFound`] if the scanner returns no rows for the symbol,
/// or [`Error::JsonParse`] if the response row is short, malformed, or contains non-finite values.
#[tracing::instrument]
pub async fn get_technical_analysis(symbol: &str) -> Result<TechnicalAnalysis> {
    let client = http_client();
    let req = ScanRequest {
        symbols: ScanSymbols {
            tickers: vec![symbol],
        },
        columns: &TA_COLUMNS,
    };

    let resp = client
        .post(TA_SCAN_URL)
        .json(&req)
        .send()
        .await
        .map_err(|e| {
            Error::Request(Ustr::from(&format!(
                "Failed to send technical analysis request: {e}"
            )))
        })?;

    if !resp.status().is_success() {
        return Err(Error::Request(Ustr::from(&format!(
            "Technical analysis request failed with status: {}",
            resp.status()
        ))));
    }

    let text = resp.text().await.map_err(|e| {
        Error::Request(Ustr::from(&format!(
            "Failed to read technical analysis response body: {e}"
        )))
    })?;

    parse_technical_analysis_response(&text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_request_body_serialization() {
        let req = ScanRequest {
            symbols: ScanSymbols {
                tickers: vec!["AMEX:SPY"],
            },
            columns: &TA_COLUMNS,
        };
        let json_val = serde_json::to_value(&req).expect("serialize scan request");
        assert_eq!(
            json_val["symbols"]["tickers"],
            serde_json::json!(["AMEX:SPY"])
        );
        let cols = json_val["columns"].as_array().expect("columns is array");
        assert_eq!(cols.len(), 24);
        assert_eq!(cols[0], "Recommend.Other|1");
        assert_eq!(cols[1], "Recommend.All|1");
        assert_eq!(cols[2], "Recommend.MA|1");
        assert_eq!(cols[15], "Recommend.Other");
        assert_eq!(cols[16], "Recommend.All");
        assert_eq!(cols[17], "Recommend.MA");
        assert_eq!(cols[21], "Recommend.Other|1M");
        assert_eq!(cols[22], "Recommend.All|1M");
        assert_eq!(cols[23], "Recommend.MA|1M");
    }

    #[test]
    fn test_technical_analysis_mapping_stable_column_order() {
        let d_values: Vec<f64> = (0..24).map(|i| (i as f64) * 0.05).collect();
        let json_payload = serde_json::json!({
            "totalCount": 1,
            "data": [{
                "s": "AMEX:SPY",
                "d": d_values
            }]
        })
        .to_string();

        let ta = parse_technical_analysis_response(&json_payload).expect("parse valid payload");

        let expected = |i: usize| normalize_recommendation((i as f64) * 0.05);

        assert_eq!(ta.period_1m.other, expected(0));
        assert_eq!(ta.period_1m.all, expected(1));
        assert_eq!(ta.period_1m.ma, expected(2));

        assert_eq!(ta.period_5m.other, expected(3));
        assert_eq!(ta.period_5m.all, expected(4));
        assert_eq!(ta.period_5m.ma, expected(5));

        assert_eq!(ta.period_15m.other, expected(6));
        assert_eq!(ta.period_15m.all, expected(7));
        assert_eq!(ta.period_15m.ma, expected(8));

        assert_eq!(ta.period_1h.other, expected(9));
        assert_eq!(ta.period_1h.all, expected(10));
        assert_eq!(ta.period_1h.ma, expected(11));

        assert_eq!(ta.period_4h.other, expected(12));
        assert_eq!(ta.period_4h.all, expected(13));
        assert_eq!(ta.period_4h.ma, expected(14));

        assert_eq!(ta.period_1d.other, expected(15));
        assert_eq!(ta.period_1d.all, expected(16));
        assert_eq!(ta.period_1d.ma, expected(17));

        assert_eq!(ta.period_1w.other, expected(18));
        assert_eq!(ta.period_1w.all, expected(19));
        assert_eq!(ta.period_1w.ma, expected(20));

        assert_eq!(ta.period_1m_month.other, expected(21));
        assert_eq!(ta.period_1m_month.all, expected(22));
        assert_eq!(ta.period_1m_month.ma, expected(23));

        assert_eq!(*ta.get(TechnicalAnalysisPeriod::Minute1), ta.period_1m);
        assert_eq!(*ta.period(Period::Day1), ta.period_1d);
        assert_eq!(ta[TechnicalAnalysisPeriod::Day1], ta.period_1d);
        assert_eq!(ta["1D"], ta.period_1d);
        assert_eq!(ta.get_by_str("1W"), Some(&ta.period_1w));
        assert_eq!(ta.get_by_str("invalid"), None);
    }

    #[test]
    fn test_technical_analysis_normalization_matches_reference() {
        assert_eq!(normalize_recommendation(0.1234), 0.246);
        assert_eq!(normalize_recommendation(-0.1234), -0.246);
        assert_eq!(normalize_recommendation(0.5), 1.0);
        assert_eq!(normalize_recommendation(-0.5), -1.0);
        assert_eq!(normalize_recommendation(1.0), 2.0);
        assert_eq!(normalize_recommendation(-1.0), -2.0);
        assert_eq!(normalize_recommendation(0.0), 0.0);
        assert_eq!(normalize_recommendation(-0.0001), 0.0);
        assert_eq!(normalize_recommendation(0.0001), 0.0);
    }

    #[test]
    fn test_technical_analysis_missing_row_fails() {
        let json_empty_data = serde_json::json!({
            "totalCount": 0,
            "data": []
        })
        .to_string();
        let res = parse_technical_analysis_response(&json_empty_data);
        match res {
            Err(Error::NoScanDataFound) => {}
            other => panic!("expected NoScanDataFound, got {:?}", other),
        }
    }

    #[test]
    fn test_technical_analysis_short_row_fails() {
        let json_short = serde_json::json!({
            "data": [{
                "s": "AMEX:SPY",
                "d": [0.1, 0.2, 0.3]
            }]
        })
        .to_string();
        let res = parse_technical_analysis_response(&json_short);
        match res {
            Err(Error::JsonParse(_)) => {}
            other => panic!("expected JsonParse, got {:?}", other),
        }
    }

    #[test]
    fn test_technical_analysis_null_value_fails() {
        let mut d_values: Vec<serde_json::Value> = (0..24)
            .map(|i| serde_json::Value::from(i as f64 * 0.1))
            .collect();
        d_values[5] = serde_json::Value::Null;
        let json_null = serde_json::json!({
            "data": [{
                "s": "AMEX:SPY",
                "d": d_values
            }]
        })
        .to_string();
        let res = parse_technical_analysis_response(&json_null);
        match res {
            Err(Error::JsonParse(_)) => {}
            other => panic!("expected JsonParse, got {:?}", other),
        }
    }

    #[test]
    fn test_technical_analysis_non_finite_value_fails() {
        let mut d_values: Vec<serde_json::Value> = (0..24)
            .map(|i| serde_json::Value::from(i as f64 * 0.1))
            .collect();
        d_values[10] = serde_json::json!("NaN");
        let json_non_numeric = serde_json::json!({
            "data": [{
                "s": "AMEX:SPY",
                "d": d_values
            }]
        })
        .to_string();
        let res = parse_technical_analysis_response(&json_non_numeric);
        match res {
            Err(Error::JsonParse(_)) => {}
            other => panic!("expected JsonParse, got {:?}", other),
        }
    }

    #[test]
    fn test_technical_analysis_public_reexport() {
        use crate::client::misc::{
            Period, TechnicalAnalysis, TechnicalAnalysisRecommendation, get_technical_analysis,
        };
        use crate::{Period as RootPeriod, TechnicalAnalysis as RootTA};

        let rec = TechnicalAnalysisRecommendation::default();
        assert_eq!(rec.all, 0.0);
        let ta = TechnicalAnalysis::default();
        assert_eq!(ta.period_1m.all, 0.0);
        let _root_ta = RootTA::default();
        assert_eq!(Period::Minute1.as_str(), "1");
        assert_eq!(RootPeriod::Minute1.as_str(), "1");
        let _fn_ptr = get_technical_analysis;
    }
}
