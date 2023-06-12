use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;

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
pub async fn get_builtin_indicators() -> Result<Vec<Indicator>, Box<dyn std::error::Error>> {
    let indicator_types = vec!["standard", "candlestick", "fundamental"];
    let mut indicators: Vec<Indicator> = vec![];
    for indicator_type in indicator_types {
        let url = format!(
            "https://pine-facade.tradingview.com/pine-facade/list/?filter={}",
            indicator_type
        );

        let mut data = crate::utils::client::get_request(&url, None)
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
    let data = crate::utils::client::get_request(
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
