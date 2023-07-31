use std::collections::HashMap;

use crate::{
    model::{Indicator, SimpleTA, Symbol, SymbolSearch},
    prelude::*,
    user::User,
};
use reqwest::Response;

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

pub struct Client {
    user: User,
}

impl Client {
    pub fn new(user: User) -> Self {
        Self { user: user }
    }

    async fn get(&self, url: &str) -> Result<Response> {
        let cookie = format!(
            "sessionid={}; sessionid_sign={};",
            self.user.session, self.user.signature
        );
        let client: reqwest::Client = crate::utils::build_request(Some(&cookie))?;
        let response = client.get(url).send().await?;
        Ok(response)
    }

    async fn post<T: serde::Serialize>(&self, url: &str, body: T) -> Result<Response> {
        let cookie = format!(
            "sessionid={}; sessionid_sign={};",
            self.user.session, self.user.signature
        );
        let client: reqwest::Client = crate::utils::build_request(Some(&cookie))?;
        let response = client.post(url).json(&body).send().await?;
        Ok(response)
    }

    #[tracing::instrument(skip(self))]
    async fn fetch_scan_data(
        &self,
        tickers: Vec<String>,
        screener: Screener,
        columns: Vec<String>,
    ) -> Result<serde_json::Value> {
        let resp_body: serde_json::Value = self
            .post(
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

    #[tracing::instrument(skip(self))]
    pub async fn get_ta(&self, exchange: &str, symbols: &[&str]) -> Result<Vec<SimpleTA>> {
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

        match self.fetch_scan_data(symbols, screener, cols.clone()).await {
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
                            let val =
                                (val.as_f64().unwrap_or(0.0) * 1000.0 / 500.0).round() / 1000.0;
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

    pub async fn search_symbol(
        &self,
        search: &str,
        exchange: &str,
        search_type: &str,
        start: u64,
        country: &str,
    ) -> Result<SymbolSearch> {
        let search_data: SymbolSearch = self
            .get(&format!(
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

    pub async fn list_symbols(&self, market_type: Option<&str>) -> Result<Vec<Symbol>> {
        match market_type {
            Some(s) => {
                let search_symbol = self.search_symbol("", "", s, 0, "").await?;
                let remaining = search_symbol.remaining;
                let mut symbols = search_symbol.symbols;
                for i in (50..remaining).step_by(50) {
                    let search_symbol = self.search_symbol("", "", s, i, "").await?;
                    symbols.extend(search_symbol.symbols);
                }
                Ok(symbols)
            }
            None => {
                let search_symbol = self.search_symbol("", "", "", 0, "").await?;
                let remaining = search_symbol.remaining;
                let mut symbols = search_symbol.symbols;
                for i in (50..remaining).step_by(50) {
                    let search_symbol = self.search_symbol("", "", "", i, "").await?;
                    symbols.extend(search_symbol.symbols);
                }
                Ok(symbols)
            }
        }
    }

    ///TODO: Implement this
    #[tracing::instrument(skip(self))]
    pub async fn get_chart_token(&self, layout_id: &str) -> Result<String> {
        let data: serde_json::Value = self
            .get(&format!(
                "https://www.tradingview.com/chart-token/?image_url={}&user_id={}",
                layout_id, self.user.id
            ))
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

    ///TODO: Implement this
    #[tracing::instrument(skip(self))]
    pub async fn get_drawing<T>(
        &self,
        layout_id: &str,
        symbol: &str,
        chart_id: T,
    ) -> Result<serde_json::Value>
    where
        T: std::fmt::Display + std::fmt::Debug,
    {
        let token = self.get_chart_token(layout_id).await.unwrap();
        let url = format!(
            "https://charts-storage.tradingview.com/charts-storage/layout/{layout_id}/sources?chart_id={chart_id}&jwt={token}&symbol={symbol}",
            layout_id = layout_id,
            chart_id = chart_id,
            token = token,
            symbol = symbol
        );

        Ok(self.get(&url).await?.json().await?)
    }

    ///TODO: Implement this
    #[tracing::instrument(skip(self))]
    pub async fn get_private_indicators(&self) -> Result<Vec<Indicator>> {
        let indicators = self
            .get("https://pine-facade.tradingview.com/pine-facade/list?filter=saved")
            .await?
            .json::<Vec<Indicator>>()
            .await?;
        Ok(indicators)
    }

    ///TODO: Implement this
    #[tracing::instrument(skip(self))]
    pub async fn get_builtin_indicators(&self) -> Result<Vec<Indicator>> {
        let indicator_types = vec!["standard", "candlestick", "fundamental"];
        let mut indicators: Vec<Indicator> = vec![];
        for indicator_type in indicator_types {
            let url = format!(
                "https://pine-facade.tradingview.com/pine-facade/list/?filter={}",
                indicator_type
            );

            let mut data = self.get(&url).await?.json::<Vec<Indicator>>().await?;

            indicators.append(&mut data);
        }
        Ok(indicators)
    }

    ///TODO: Implement this
    #[tracing::instrument(skip(self))]
    pub async fn get_indicator_data(&self, indicator: &Indicator) -> Result<serde_json::Value> {
        let url = format!(
            "https://pine-facade.tradingview.com/pine-facade/translate/{}/{}",
            indicator.id, indicator.version
        );
        let data: serde_json::Value = self.get(&url).await?.json().await?;
        Ok(data)
    }
}
