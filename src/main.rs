use reqwest::get;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = "https://apipubaws.tcbs.com.vn/tcanalysis/v1/ticker/FPT/overview";
    let res = get(url).await.unwrap();
    let data: serde_json::Value = res.json().await.unwrap();
    dbg!(data);
    Ok(())
}
