use tracing::info;

use serde::Deserialize;

pub type Root = Vec<Root2>;

#[derive(Default, Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Root2 {
    pub user_id: i64,
    pub script_name: String,
    pub script_source: String,
    pub script_access: String,
    pub script_id_part: String,
    pub version: String,
    pub extra: Extra,
}

#[derive(Default, Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Extra {
    pub financial_period: Option<String>,
    #[serde(rename = "fund_id")]
    pub fund_id: String,
    pub fundamental_category: String,
    pub is_fundamental_study: bool,
    #[serde(rename = "is_hidden_study")]
    pub is_hidden_study: bool,
    pub kind: String,
    pub short_description: String,
    pub source_inputs_count: i64,
    pub tags: Vec<String>,
}

#[tracing::instrument]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting up");
    let url = "https://pine-facade.tradingview.com//pine-facade/list/?filter=fundamental";

    let client = reqwest::Client::new();

    let res = client.get(url).send().await?.json::<Root>().await?;

    // let res = get(url).await.unwrap();
    // let data: serde_json::Value = res.json().await.unwrap();
    dbg!(res);
    Ok(())
}
