pub mod utils;

use tracing::info;

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
    pub description: Extra,
}

#[derive(Default, Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Extra {
    pub kind: String,
    pub short_description: String,
}

#[tracing::instrument]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting up");
    let indicator_types = vec!["standard", "candlestick", "fundamental"];
    let mut indicators: Vec<Indicator> = vec![];
    for indicator_type in indicator_types {
        info!("Fetching indicators for type: {}", indicator_type);
        let url = format!(
            "https://pine-facade.tradingview.com//pine-facade/list/?filter={}",
            indicator_type
        );

        let mut data = utils::client::get(&url)
            .await?
            .json::<Vec<Indicator>>()
            .await?;

        indicators.append(&mut data);
    }

    dbg!(indicators.len());
    Ok(())
}
