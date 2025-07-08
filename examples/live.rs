use dotenv::dotenv;
use std::{env, sync::Arc, time::Duration};
use tokio::{
    sync::mpsc,
    time::{sleep, timeout},
};
use tracing::{error, info, warn};
use ustr::ustr;

use tradingview::{
    Interval, OHLCV,
    chart::ChartOptions,
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
    tracing_subscriber::fmt::init();

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

    // Spawn command runner task
    let runner_handle = tokio::spawn(async move {
        if let Err(e) = command_runner.run().await {
            error!("Command runner failed: {}", e);
        }
        info!("Command runner stopped");
    });

    // Spawn data response handler
    let response_handle = tokio::spawn(async move {
        handle_responses(response_rx).await;
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
    let shutdown_timeout = Duration::from_secs(10);

    if let Err(_) = timeout(shutdown_timeout, runner_handle).await {
        warn!("Command runner did not shutdown gracefully within timeout");
    }

    if let Err(_) = timeout(shutdown_timeout, response_handle).await {
        warn!("Response handler did not shutdown gracefully within timeout");
    }

    info!("Shutdown complete");
    Ok(())
}

async fn handle_responses(mut response_rx: DataRx) {
    info!("Response handler started");

    while let Some(response) = response_rx.recv().await {
        match response {
            TradingViewResponse::ChartData(series_info, data_points) => {
                info!(
                    "📊 Chart Data - Session: {}, Points: {}",
                    series_info.chart_session,
                    data_points.len()
                );

                // Print last few data points
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

    info!("Response handler stopped");
}

#[allow(dead_code)]
async fn run_full_command(command_tx: CommandTx) -> anyhow::Result<()> {
    info!("🚀 Starting trading example...");

    let auth_token = env::var("TV_AUTH_TOKEN").expect("TV_AUTH_TOKEN is not set");

    // Wait a bit for WebSocket to establish connection
    sleep(Duration::from_secs(2)).await;

    // Step 1: Set up authentication (if needed to refresh)
    info!("1️⃣ Setting up authentication...");
    command_tx.send(Command::SetAuthToken {
        auth_token: ustr(&auth_token),
    })?;

    // Step 2: Set locale and data quality
    info!("2️⃣ Configuring locale and data quality...");
    command_tx.send(Command::SetLocals {
        language: ustr("en"),
        country: ustr("US"),
    })?;

    // command_tx.send(Command::SetDataQuality {
    //     quality: ustr("streaming"),
    // })?;

    // Step 3: Set up quote session for real-time quotes
    info!("3️⃣ Setting up quote session...");
    command_tx.send(Command::CreateQuoteSession)?;

    sleep(Duration::from_millis(500)).await;

    command_tx.send(Command::SetQuoteFields)?;

    // Step 4: Add symbols for quote monitoring
    info!("4️⃣ Adding symbols for quote monitoring...");
    command_tx.send(Command::QuoteFastSymbols {
        symbols: vec![
            ustr("NASDAQ:AAPL"),
            ustr("NASDAQ:MSFT"),
            ustr("NASDAQ:GOOGL"),
        ],
    })?;

    // Step 5: Set up chart data
    info!("5️⃣ Setting up chart data...");
    let chart_options = ChartOptions::builder()
        .symbol("AAPL".into())
        .exchange("NASDAQ".into())
        .interval(Interval::OneDay)
        .bar_count(100)
        .build();

    command_tx.send(Command::SetMarket {
        options: chart_options,
    })?;

    // Step 6: Send periodic pings and monitor connection
    info!("6️⃣ Starting monitoring loop...");
    let mut ping_counter = 0;

    for i in 0..60 {
        // Run for 60 iterations (about 5 minutes with 5-second intervals)
        sleep(Duration::from_secs(5)).await;

        // Send ping every 30 seconds
        if i % 6 == 0 {
            ping_counter += 1;
            info!("📡 Sending ping #{}", ping_counter);
            command_tx.send(Command::Ping)?;
        }

        // Add more symbols periodically
        if i == 20 {
            info!("➕ Adding more symbols...");
            command_tx.send(Command::QuoteFastSymbols {
                symbols: vec![ustr("NYSE:TSLA"), ustr("NASDAQ:AMZN")],
            })?;
        }

        // Remove some symbols later
        if i == 40 {
            info!("➖ Removing some symbols...");
            command_tx.send(Command::QuoteRemoveSymbols {
                symbols: vec![ustr("NASDAQ:GOOGL")],
            })?;
        }

        // Request more historical data
        if i == 30 {
            info!("📊 Requesting more historical data...");
            command_tx.send(Command::RequestMoreData {
                session: ustr("cs_1"), // This should match your actual chart session
                series_id: ustr("sds_1"),
                bar_count: 50,
            })?;
        }

        info!("⏰ Monitoring iteration {} completed", i + 1);
    }

    // Step 7: Cleanup
    info!("7️⃣ Cleaning up...");
    command_tx.send(Command::DeleteQuoteSession)?;

    sleep(Duration::from_secs(1)).await;

    command_tx.send(Command::Delete)?;

    info!("✅ Trading example completed successfully!");
    Ok(())
}

// Alternative simpler example for quick testing
#[allow(dead_code)]
async fn run_simple_example(command_tx: CommandTx) -> anyhow::Result<()> {
    info!("🔧 Running simple example...");

    // Just set up basic quote monitoring
    command_tx.send(Command::CreateQuoteSession)?;
    // sleep(Duration::from_millis(500)).await;

    command_tx.send(Command::SetQuoteFields)?;
    // sleep(Duration::from_millis(500)).await;

    command_tx.send(Command::QuoteFastSymbols {
        symbols: vec![ustr("BINANCE:BTCUSDT")],
    })?;

    // Monitor for 30 seconds
    // for i in 0..6 {
    //     sleep(Duration::from_secs(5)).await;
    //     info!("⏰ Simple monitoring {} of 6", i + 1);

    //     if i % 2 == 0 {
    //         command_tx.send(Command::Ping)?;
    //     }
    // }

    // command_tx.send(Command::DeleteQuoteSession)?;
    // command_tx.send(Command::Delete)?;

    info!("✅ Simple example completed!");
    Ok(())
}
