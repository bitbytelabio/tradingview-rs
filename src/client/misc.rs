use crate::{
    ChartDrawing, Country, CryptoCentralization, EconomicCategory, EconomicSource,
    FuturesProductType, MarketType, Result, StockSector, Symbol, SymbolSearchResponse, UserCookies,
    error::Error,
    pine_indicator::{self, BuiltinIndicators, PineInfo, PineMetadata, PineSearchResult},
    utils::build_request,
};
use bon::builder;
use reqwest::Response;
use serde_json::Value;
use std::sync::Arc;
use tokio::{sync::Semaphore, task::JoinHandle};
use tracing::debug;
use urlencoding::encode;

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

/// Sends an HTTP GET request to the specified URL using the provided client and returns the response.
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
    if let Some(client) = client {
        let cookie = format!(
            "sessionid={}; sessionid_sign={}; device_t={};",
            client.session, client.session_signature, client.device_token
        );
        let client = build_request(Some(&cookie))?;
        let response = client.get(url).send().await?;
        return Ok(response);
    }
    Ok(build_request(None)?.get(url).send().await?)
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
        return Err(Error::Generic(
            "URL too long - please reduce search parameters".to_string(),
        ));
    }

    let search_data: SymbolSearchResponse = get(None, &url)
        .await
        .map_err(|e| Error::Generic(format!("Failed to fetch symbol search: {}", e)))?
        .json()
        .await
        .map_err(|e| Error::Generic(format!("Failed to parse symbol search response: {}", e)))?;

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
            let _permit = semaphore
                .acquire()
                .await
                .map_err(|e| Error::Generic(format!("Failed to acquire semaphore: {}", e)))?;

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
                    Error::Generic(format!("Failed to fetch symbols at offset {}: {}", i, e))
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
                return Err(Error::Generic(format!(
                    "Task {} panicked: {}",
                    index, join_err
                )));
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
        return Err(Error::Generic("No results found".to_string()));
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

    Err(Error::Generic(
        "Failed to get indicator metadata".to_string(),
    ))
}
