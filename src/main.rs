use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    // tradingview_rs::misc_requests::get_builtin_indicators().await?;

    let ind = tradingview_rs::misc_requests::Indicator {
        name: "Accounts payable".to_string(),
        id: "STD;Fund_accounts_payable_fy".to_string(),
        version: "52.0".to_string(),
        info: HashMap::new(),
    };

    tradingview_rs::misc_requests::get_indicator_data(&ind).await;
    Ok(())
}
