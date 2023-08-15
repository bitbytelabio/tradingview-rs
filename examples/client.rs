use std::env;

use tracing::info;
use tradingview_rs::{client::mics::*, models::pine_indicator::*, user::User};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let session = env::var("TV_SESSION").unwrap();
    let signature = env::var("TV_SIGNATURE").unwrap();

    let user = User::build()
        .session(&session, &signature)
        .get()
        .await
        .unwrap();

    let indicators = get_builtin_indicators(&user, BuiltinIndicators::Standard)
        .await
        .unwrap();

    let info = indicators.first().unwrap();
    info!("{:#?}", info);

    let metadata = get_indicator_metadata(&user, &info).await.unwrap();
    info!("{:#?}", metadata);
}
