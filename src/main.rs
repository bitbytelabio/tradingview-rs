use reqwest::header::USER_AGENT;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let res = client
        .get("https://http3.is")
        .version(reqwest::Version::HTTP_3)
        .header(USER_AGENT, "reqwest")
        .send()
        .await?;
    println!("{:#?}", res);
    Ok(())
}
