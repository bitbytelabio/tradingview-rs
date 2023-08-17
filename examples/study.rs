use std::env;

use tracing::info;
use tradingview_rs::{
    chart::{
        session::{ChartCallbackFn, WebSocket},
        ChartOptions, ChartSeries, StudyOptions,
    },
    models::Interval,
    socket::DataServer,
    user::User,
};

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

    let handlers = ChartCallbackFn {
        on_chart_data: Box::new(|data| Box::pin(on_chart_data(data))),
        on_symbol_resolved: Box::new(|data| Box::pin(on_symbol_resolved(data))),
        on_series_completed: Box::new(|data| Box::pin(on_series_completed(data))),
    };

    let mut socket = WebSocket::build()
        .server(DataServer::ProData)
        .auth_token(user.auth_token)
        .connect(handlers)
        .await
        .unwrap();

    socket
        .set_market(
            "HOSE:FPT",
            ChartOptions {
                resolution: Interval::Daily,
                bar_count: 1,
                study_config: Some(StudyOptions {
                    script_id: "STD;Fund_total_revenue_fq".to_string(),
                    script_version: "62.0".to_string(),
                    script_type: tradingview_rs::models::pine_indicator::ScriptType::IntervalScript,
                }),
                ..Default::default()
            },
        )
        .await
        .unwrap();

    socket.subscribe().await;
}

async fn on_chart_data(_data: ChartSeries) -> Result<(), tradingview_rs::error::Error> {
    // info!("on_chart_data: {:?}", data);
    // let end = data.data.first().unwrap().0;
    // info!("on_chart_data: {:?} - {:?}", data.data.len(), end);
    Ok(())
}

async fn on_symbol_resolved(
    data: tradingview_rs::chart::SymbolInfo,
) -> Result<(), tradingview_rs::error::Error> {
    info!("on_symbol_resolved: {:?}", data);
    Ok(())
}

async fn on_series_completed(
    data: tradingview_rs::chart::SeriesCompletedMessage,
) -> Result<(), tradingview_rs::error::Error> {
    info!("on_series_completed: {:?}", data);
    Ok(())
}
