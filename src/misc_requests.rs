use serde::Deserialize;
#[derive(Default, Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Indicator {
    #[serde(rename = "scriptName")]
    pub name: String,
    #[serde(rename = "scriptIdPart")]
    pub id: String,
    pub version: String,
    #[serde(rename = "extra")]
    pub info: ExtraIndicatorInfo,
}

#[derive(Default, Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtraIndicatorInfo {
    pub kind: String,
    pub short_description: String,
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
