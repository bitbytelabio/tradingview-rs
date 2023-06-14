use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    // tradingview_rs::misc_requests::get_builtin_indicators().await?;

    // let ind = tradingview_rs::misc_requests::Indicator {
    //     name: "Accounts payable".to_string(),
    //     id: "STD;Fund_accounts_payable_fy".to_string(),
    //     version: "52.0".to_string(),
    //     info: HashMap::new(),
    // };

    // tradingview_rs::misc_requests::get_indicator_data(&ind).await;
    let user_data = tradingview_rs::auth::get_user(
        "d0pbunzgjclwd2lqe8qixy014hhxdszh",
        "v1:EIhjnp50X5rLMlIlVNHlpcCi8D4hYMNHAvKVnjXr8mA=",
        None,
    )
    .await?;

    tradingview_rs::misc_requests::get_drawing("FiwrRse6", "TSLA", &user_data, 1).await;

    Ok(())
}
