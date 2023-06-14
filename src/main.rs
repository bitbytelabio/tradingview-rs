#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    tradingview_rs::misc_requests::get_builtin_indicators().await?;
    Ok(())
}
