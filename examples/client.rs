use tracing::info;
use tradingview_rs::{client::mics::*, models::pine_indicator::*};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let indicators = get_builtin_indicators(BuiltinIndicators::Fundamental)
        .await
        .unwrap();

    let info = indicators.first().unwrap();
    info!("{:#?}", info);

    let metadata = get_indicator_metadata(None, &info.script_id, &info.script_version)
        .await
        .unwrap();
    info!("{:#?}", metadata);

    let pine = PineIndicator {
        pine_type: PineType::Script,
        info: info.clone(),
        options: metadata,
    };

    let test = pine.set_study_input();

    info!("{:#?}", test);

    let json_string = serde_json::to_string(&test).unwrap();
    println!("{}", json_string);
}
