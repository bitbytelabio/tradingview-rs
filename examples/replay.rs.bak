use dotenv::dotenv;
use std::env;
use tradingview::{Interval, history, socket::DataServer};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let auth_token = env::var("TV_AUTH_TOKEN").expect("TV_AUTH_TOKEN is not set");

    let (_info, data) = history::single::retrieve()
        .auth_token(&auth_token)
        .symbol("BTCUSDT")
        .exchange("OKX")
        .interval(Interval::OneHour)
        .server(DataServer::ProData)
        .with_replay(true)
        .call()
        .await?;

    println!("Data length: {}", data.len());

    Ok(())
}
