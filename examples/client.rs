use tracing::info;
use tradingview_rs::{client::mics::*, models::pine_indicator::*};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let indicators = get_builtin_indicators(BuiltinIndicators::Standard)
        .await
        .unwrap();

    let info = indicators.first().unwrap();
    info!("{:#?}", info);

    let metadata = get_indicator_metadata(None, &info).await.unwrap();
    info!("{:#?}", metadata);
}
