#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let user =
        tradingview_rs::auth::login_user("batttheyshool0211", "batttheyshool0211", None).await?;
    println!("{:#?}", user);
    Ok(())
}
