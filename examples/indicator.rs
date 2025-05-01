use anyhow::Ok;
use dotenv::dotenv;
use std::env;
use tradingview::{
    Interval,
    callback::Callbacks,
    chart::ChartOptions,
    get_builtin_indicators,
    pine_indicator::{BuiltinIndicators, ScriptType},
    socket::DataServer,
    websocket::{WebSocket, WebSocketClient},
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    tracing_subscriber::fmt::init();
    let auth_token = env::var("TV_AUTH_TOKEN").expect("TV_AUTH_TOKEN is not set");

    let buildins = get_builtin_indicators(BuiltinIndicators::Fundamental).await?;
    println!("{:#?}", buildins[0]);
    let study_callback = |data| {
        println!("{:#?}", data);
    };
    let callbacks: Callbacks = Callbacks::default().on_study_data(study_callback);

    let client = WebSocketClient::default().set_callbacks(callbacks);

    let websocket = WebSocket::new()
        .server(DataServer::ProData)
        .auth_token(&auth_token)
        .client(client)
        .build()
        .await?;

    let opts = ChartOptions::new_with("ACV", "UPCOM", Interval::Daily)
        .bar_count(1)
        .study_config(
            &buildins[0].script_id,
            &buildins[0].script_version,
            ScriptType::IntervalScript,
        );

    websocket.set_market(opts).await?;

    websocket.subscribe().await;

    Ok(())
}
