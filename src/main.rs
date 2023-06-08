use reqwest::header::HeaderMap;
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
        let client = reqwest::Client::new();
        let url = format!(
            "https://pine-facade.tradingview.com//pine-facade/list/?filter={}",
            indicator_type
        );
        let default_headers = vec![
            ("Accept", "application/json, text/plain, */*"),
            ("Accept-Encoding", "gzip, deflate, br"),
            ("Accept-Language", "en-US,en;q=0.9"),
            ("Connection", "keep-alive"),
            ("Origin", "https://www.tradingview.com"),
            ("Referer", "https://www.tradingview.com/"),
            ("Sec-Fetch-Dest", "empty"),
            ("Sec-Fetch-Mode", "cors"),
            ("Sec-Fetch-Site", "same-site"),
            ("User-Agent", "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/112.0.0.0 Safari/537.36 uacq"),
        ];
        let mut headers = HeaderMap::new();
        for (key, value) in default_headers {
            headers.insert(key, value.parse().unwrap());
        }
        // let test = reqwest::ClientBuilder::new()
        //     .default_headers(headers)
        //     .build()?;
        let mut data = client
            .get(url)
            .headers(headers)
            .send()
            .await?
            .json::<Vec<Indicator>>()
            .await?;

        indicators.append(&mut data);
    }

    dbg!(indicators);
    Ok(())
}
