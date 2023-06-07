use reqwest::{get, Client, IntoUrl, Response, Version};
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = "https://apipubaws.tcbs.com.vn/tcanalysis/v1/ticker/FPT/overview";
    let res = get(url).await?.json::<HashMap<String, String>>().await?;
    println!("body = {:?}", res);
    Ok(())
}
