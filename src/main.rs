use reqwest::{Client, IntoUrl, Response, Version};

async fn get<T: IntoUrl + Clone>(url: T) -> reqwest::Result<Response> {
    Client::builder()
        .http2_prior_knowledge()
        .build()?
        .get(url)
        .version(Version::HTTP_2)
        .send()
        .await
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let res = get("https://httpbin.org/ip").await?;
    let body = res.json().await?;
    println!("body = {:?}", body);
    Ok(())
}
