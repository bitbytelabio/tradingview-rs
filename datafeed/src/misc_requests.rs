use crate::utils::get_request;
use reqwest::{Client, Error};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use tracing::{debug, error, info, warn};

#[derive(Default, Debug, Clone, PartialEq, Deserialize)]
pub struct Indicator {
    #[serde(rename = "scriptName")]
    pub name: String,

    #[serde(rename = "scriptIdPart")]
    pub id: String,

    pub version: String,

    #[serde(flatten, rename = "extra")]
    pub info: HashMap<String, Value>,
}

#[tracing::instrument]
async fn fetch_scan_data(
    tickers: Vec<String>,
    scan_type: &str,
    columns: Vec<String>,
) -> Result<serde_json::Value, Error> {
    let client = Client::new();
    let url = format!("https://scanner.tradingview.com/{}/scan", scan_type);
    let body = serde_json::json!({
        "symbols": {
            "tickers": tickers
        },
        "columns": columns
    });

    let response = client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await?;

    let data = response.text().await?;

    if !data.starts_with('{') {
        Err("Wrong screener or symbol").unwrap()
    }

    let parsed_data = serde_json::from_str(&data).unwrap();

    Ok(parsed_data)
}

#[tracing::instrument]
pub async fn get_indicator_data(indicator: &Indicator) {
    let data: Value = get_request(
        format!(
            "https://pine-facade.tradingview.com/pine-facade/translate/{}/{}",
            indicator.id, indicator.version
        )
        .as_str(),
        None,
    )
    .await
    .unwrap()
    .json()
    .await
    .unwrap();
    debug!("{:?}", data);
    // !TODO: add parsing for indicator data
}

#[tracing::instrument]
pub async fn get_builtin_indicators() -> Result<Vec<Indicator>, Box<dyn std::error::Error>> {
    let indicator_types = vec!["standard", "candlestick", "fundamental"];
    let mut indicators: Vec<Indicator> = vec![];
    for indicator_type in indicator_types {
        let url = format!(
            "https://pine-facade.tradingview.com/pine-facade/list/?filter={}",
            indicator_type
        );

        let mut data = get_request(&url, None)
            .await?
            .json::<Vec<Indicator>>()
            .await?;

        indicators.append(&mut data);
    }
    Ok(indicators)
}

#[tracing::instrument]
pub async fn get_private_indicators(
    session: &str,
    signature: &str,
) -> Result<Vec<Indicator>, Box<dyn std::error::Error>> {
    let data = get_request(
        "https://pine-facade.tradingview.com/pine-facade/list?filter=saved",
        Some(format!(
            "sessionid={}; sessionid_sign={};",
            session, signature
        )),
    )
    .await?
    .json::<Vec<Indicator>>()
    .await?;
    Ok(data)
}

#[tracing::instrument]
pub fn get_screener(exchange: &str) -> String {
    let e = exchange.to_uppercase();
    if ["NASDAQ", "NYSE", "NYSE ARCA", "OTC"].contains(&e.as_str()) {
        return String::from("america");
    }
    if ["ASX"].contains(&e.as_str()) {
        return String::from("australia");
    }
    if ["TSX", "TSXV", "CSE", "NEO"].contains(&e.as_str()) {
        return String::from("canada");
    }
    if ["EGX"].contains(&e.as_str()) {
        return String::from("egypt");
    }
    if ["FWB", "SWB", "XETR"].contains(&e.as_str()) {
        return String::from("germany");
    }
    if ["BSE", "NSE"].contains(&e.as_str()) {
        return String::from("india");
    }
    if ["TASE"].contains(&e.as_str()) {
        return String::from("israel");
    }
    if ["MIL", "MILSEDEX"].contains(&e.as_str()) {
        return String::from("italy");
    }
    if ["LUXSE"].contains(&e.as_str()) {
        return String::from("luxembourg");
    }
    if ["NEWCONNECT"].contains(&e.as_str()) {
        return String::from("poland");
    }
    if ["NGM"].contains(&e.as_str()) {
        return String::from("sweden");
    }
    if ["BIST"].contains(&e.as_str()) {
        return String::from("turkey");
    }
    if ["LSE", "LSIN"].contains(&e.as_str()) {
        return String::from("uk");
    }
    if ["HNX", "HOSE", "UPCOM"].contains(&e.as_str()) {
        return String::from("vietnam");
    }
    return exchange.to_lowercase();
}

#[tracing::instrument(skip(user_data))]
pub async fn get_chart_token(
    layout_id: &str,
    user_data: Option<&crate::auth::UserData>,
) -> Result<String, Box<dyn std::error::Error>> {
    match user_data {
        Some(user) => {
            let res = get_request(
                format!(
                    "https://www.tradingview.com/chart-token/?image_url={}&user_id={}",
                    layout_id, user.id
                )
                .as_str(),
                Some(
                    format!(
                        "sessionid={}; sessionid_sign={};",
                        user.session, user.signature
                    )
                    .to_string(),
                ),
            )
            .await
            .unwrap();
            let data: Value = res.json().await.unwrap();
            match data.get("token") {
                Some(token) => return Ok(token.as_str().unwrap().to_string()),
                None => return Err("No token found").unwrap(),
            }
        }
        None => {
            error!("No user data provided");
            Err("No user data provided".into())
        }
    }
}

#[tracing::instrument(skip(user_data))]
pub async fn get_drawing<T>(
    layout_id: &str,
    symbol: &str,
    user_data: &crate::auth::UserData,
    chart_id: T,
) where
    T: std::fmt::Display + std::fmt::Debug,
{
    let token = get_chart_token(layout_id, Some(user_data)).await.unwrap();
    let url = format!(
        "https://charts-storage.tradingview.com/charts-storage/layout/{layout_id}/sources?chart_id={chart_id}&jwt={token}&symbol={symbol}"  
    );
    let res = get_request(
        url.as_str(),
        Some(format!(
            "sessionid={}; sessionid_sign={};",
            user_data.session, user_data.signature
        )),
    )
    .await
    .unwrap();
    let data: Value = res.json().await.unwrap();
    info!("{:?}", data);
    // !TODO: add parsing for chart drawing data
    // info!("{}", token);
}
