use tracing::info;
use tradingview::pine_indicator::*;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    // let indicators = get_builtin_indicators(BuiltinIndicators::Fundamental)
    //     .await
    //     .unwrap();

    // let info = indicators.first().unwrap();
    // info!("{:#?}", info);

    let pine = PineIndicator::build()
        .fetch("STD;Fund_total_revenue_fq", "62.0", ScriptType::Script)
        .await
        .unwrap();

    let test = pine.to_study_inputs().unwrap();

    info!("{:#?}", test);

    let json_string = serde_json::to_string(&test).unwrap();
    println!("{json_string}");
}
