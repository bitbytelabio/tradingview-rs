use dotenv::dotenv;
use std::env;
use tracing::info;
use tradingview::{
    chart::{
        session::{ChartCallbackFn, WebSocket},
        ChartOptions, ChartSeries, StudyOptions,
    },
    models::Interval,
    socket::DataServer,
};

#[tokio::main]
async fn main() {
    dotenv().ok();
    tracing_subscriber::fmt::init();

    let auth_token = env::var("TV_AUTH_TOKEN").unwrap();

    let handlers = ChartCallbackFn {
        on_chart_data: Box::new(|data| Box::pin(on_chart_data(data))),
        on_symbol_resolved: Box::new(|data| Box::pin(on_symbol_resolved(data))),
        on_series_completed: Box::new(|data| Box::pin(on_series_completed(data))),
    };

    let mut socket = WebSocket::build()
        .server(DataServer::ProData)
        .auth_token(auth_token)
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
                    script_type: tradingview::models::pine_indicator::ScriptType::IntervalScript,
                }),
                ..Default::default()
            },
        )
        .await
        .unwrap();

    socket.subscribe().await;
}

async fn on_chart_data(_data: ChartSeries) -> Result<(), tradingview::error::Error> {
    // info!("on_chart_data: {:?}", data);
    // let end = data.data.first().unwrap().0;
    // info!("on_chart_data: {:?} - {:?}", data.data.len(), end);
    Ok(())
}

async fn on_symbol_resolved(
    data: tradingview::chart::SymbolInfo,
) -> Result<(), tradingview::error::Error> {
    info!("on_symbol_resolved: {:?}", data);
    Ok(())
}

async fn on_series_completed(
    data: tradingview::chart::SeriesCompletedMessage,
) -> Result<(), tradingview::error::Error> {
    info!("on_series_completed: {:?}", data);
    Ok(())
}
