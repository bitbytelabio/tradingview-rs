use dotenv::dotenv;
use std::{env, sync::Arc, time::Duration};
use tokio::{
    sync::mpsc,
    time::{sleep, timeout},
};
use tracing::{error, info, warn};
use tradingview::{
    ChartOptions, Interval, OHLCV,
    live::{
        handler::{
            command::CommandRunner,
            message::{Command, TradingViewResponse},
            types::{CommandTx, DataRx},
        },
        models::DataServer,
        websocket::WebSocketClient,
    },
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

    info!("WebSocket client created successfully");

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
                                info!(
                                    "📊 Chart Data - Session: {}, Points: {}",
                                    series_info.chart_session,
                                    data_points.len()
                                );

                                if !data_points.is_empty() {
                                    let last_point = &data_points[data_points.len() - 1];
                                    info!(
                                        "   Latest: Time: {}, Open: {}, High: {}, Low: {}, Close: {}, Volume: {}",
                                        last_point.timestamp(),
                                        last_point.open(),
                                        last_point.high(),
                                        last_point.low(),
                                        last_point.close(),
                                        last_point.volume()
                                    );
                                }
                            }

                            TradingViewResponse::QuoteData(quote) => {
                                info!("💱 Quote Data: {:#?}", quote);
                            }

                            TradingViewResponse::SymbolInfo(symbol_info) => {
                                info!(
                                    "ℹ️  Symbol Info: {} - {}",
                                    symbol_info.name, symbol_info.description
                                );
                            }

                            TradingViewResponse::SeriesLoading(loading_msg) => {
                                info!("⏳ Series Loading: {:?}", loading_msg);
                            }

                            TradingViewResponse::SeriesCompleted(data) => {
                                info!("✅ Series Completed: {:?}", data);
                            }

                            TradingViewResponse::StudyData(study_options, study_data) => {
                                info!(
                                    "📈 Study Data: {:?} - {} points",
                                    study_options,
                                    study_data.studies.len()
                                );
                            }

                            TradingViewResponse::Error(error, context) => {
                                error!("❌ Error: {} - Context: {:?}", error, context);
                            }

                            TradingViewResponse::UnknownEvent(event, data) => {
                                warn!("❓ Unknown Event: {} - Data: {:?}", event, data);
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

    let options = ChartOptions::builder()
        .symbol("BTCUSDT".into())
        .exchange("BINANCE".into())
        .interval(Interval::OneMinute)
        .build();

    // Try symbols that are more likely to have data during market hours
    command_tx.send(Command::add_symbol("TVC:GOLD"))?;

    command_tx.send(Command::set_market(options))?;

    // Monitor for longer - 2 minutes to give time for data
    for i in 0..24 {
        // Increased to 24 (2 minutes)
        sleep(Duration::from_secs(5)).await;
        info!("⏰ Simple monitoring {} of 24", i + 1);

        // Add more symbols at different intervals to test
        if i == 6 {
            info!("➕ Adding more symbols...");
            command_tx.send(Command::add_symbols(&[
                "NASDAQ:AAPL",
                "NASDAQ:GOOGL",
                "NYSE:TSLA",
            ]))?;
        }

        if i == 12 {
            info!("➕ Adding forex symbols...");
            command_tx.send(Command::remove_symbol("TVC:GOLD"))?;
        }
    }

    info!("🧹 Cleaning up...");
    command_tx.send(Command::DeleteQuoteSession)?;
    sleep(Duration::from_millis(500)).await;

    command_tx.send(Command::Delete)?;

    info!("✅ Simple example completed!");
    Ok(())
}
