// TODO: FIX data series loading

use dotenv::dotenv;
use std::{env, sync::Arc, time::Duration};
use tokio::{
    sync::mpsc,
    time::{sleep, timeout},
};
use tracing::{error, info, warn};
use tradingview::{
    ChartOptions, Interval, StudyOptions, get_builtin_indicators,
    live::{
        handler::{
            command::CommandRunner,
            message::{Command, TradingViewResponse},
            types::{CommandTx, DataRx},
        },
        models::DataServer,
        websocket::WebSocketClient,
    },
    pine_indicator::{BuiltinIndicators, ScriptType},
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();

    // Use debug level to see more details
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let auth_token = env::var("TV_AUTH_TOKEN").expect("TV_AUTH_TOKEN is not set");

    // Create communication channels
    let (response_tx, response_rx) = mpsc::unbounded_channel();
    let (command_tx, command_rx) = mpsc::unbounded_channel();

    info!("Starting TradingView WebSocket client...");
    // Create WebSocket client
    let ws_client = WebSocketClient::builder()
        .auth_token(&auth_token)
        .server(DataServer::ProData)
        .data_tx(response_tx)
        .build()
        .await?;

    // Create command runner
    let command_runner = CommandRunner::new(command_rx, Arc::clone(&ws_client));
    let shutdown_token = command_runner.shutdown_token();

    // Clone shutdown token for response handler
    let response_shutdown = shutdown_token.clone();

    // Spawn command runner task
    let runner_handle = tokio::spawn(async move {
        if let Err(e) = command_runner.run().await {
            error!("Command runner failed: {}", e);
        }
        info!("Command runner stopped");
    });

    // Spawn data response handler with shutdown support
    let response_handle = tokio::spawn(async move {
        tokio::select! {
            _ = handle_responses(response_rx) => {
                info!("Response handler completed");
            }
            _ = response_shutdown.cancelled() => {
                info!("Response handler received shutdown signal");
            }
        }
    });

    // Spawn example trading session
    let trading_handle = tokio::spawn(async move {
        if let Err(e) = run_simple_example(command_tx).await {
            error!("Trading example failed: {}", e);
        }
    });

    // Wait for user interruption or completion
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            info!("Received Ctrl+C, shutting down...");
        },
        _ = trading_handle => {
            info!("Trading example completed");
        }
    }

    // Graceful shutdown
    info!("Initiating graceful shutdown...");
    shutdown_token.cancel();

    // Wait for tasks to complete with timeout
    let shutdown_timeout = Duration::from_secs(5);

    let _ = timeout(shutdown_timeout, runner_handle).await;
    let _ = timeout(shutdown_timeout, response_handle).await;

    info!("Shutdown complete");

    Ok(())
}

async fn handle_responses(mut response_rx: DataRx) {
    info!("Response handler started");

    loop {
        tokio::select! {
            response = response_rx.recv() => {
                match response {
                    Some(response) => {
                        match response {
                            TradingViewResponse::ChartData(series_info, data_points) => {
                                // Study data will be send via ChartData
                                info!(
                                    "📊 Chart Data: {} - {} points",
                                    series_info.options.symbol, data_points.len()
                                );

                                for (i, point) in data_points.iter().enumerate() {
                                   warn!(
                                      "Data Point i:{} , value:{:?}",
                                        i,
                                        point
                                    );
                                }

                            }

                            TradingViewResponse::StudyData(study_options, study_data) => {
                                info!(
                                    "📈 Study Data: {:?} - {} points",
                                    study_options,
                                    study_data.studies.len()
                                );
                            }

                            _ => {
                                info!("📨 Other Response: {:?}", response);
                            }
                        }
                    }
                    None => {
                        info!("Response channel closed");
                        break;
                    }
                }
            }
        }
    }

    info!("Response handler stopped");
}

async fn run_simple_example(command_tx: CommandTx) -> anyhow::Result<()> {
    info!("🔧 Running simple example...");
    let buildins = get_builtin_indicators(BuiltinIndicators::Standard).await?;
    if buildins.is_empty() {
        error!("No built-in indicators found");
        return Ok(());
    }
    info!("Found {} built-in indicators", buildins.len());
    info!(
        "Using first built-in indicator: {} v{}",
        buildins[0].script_id, buildins[0].script_version
    );
    let opts = ChartOptions::builder()
        .symbol("BTCUSDT".into())
        .exchange("BINANCE".into())
        .interval(Interval::OneDay)
        .bar_count(20)
        .study_config(StudyOptions {
            script_id: (&buildins[0].script_id).into(),
            script_version: (&buildins[0].script_version).into(),
            script_type: ScriptType::IntervalScript,
        })
        .build();

    command_tx.send(Command::set_market(opts))?;

    // Monitor for longer - 2 minutes to give time for data
    for _ in 0..24 {
        // Increased to 24 (2 minutes)
        sleep(Duration::from_secs(5)).await;
    }

    info!("🧹 Cleaning up...");
    command_tx.send(Command::DeleteQuoteSession)?;
    sleep(Duration::from_millis(500)).await;

    command_tx.send(Command::Delete)?;

    info!("✅ Simple example completed!");
    Ok(())
}
