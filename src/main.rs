use reqwest::get;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = "https://pine-facade.tradingview.com//pine-facade/list/?filter=fundamental";
    let res = get(url).await.unwrap();
    let data: serde_json::Value = res.json().await.unwrap();
    dbg!(data);
    Ok(())
}
