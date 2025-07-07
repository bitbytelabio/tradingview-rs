use anyhow::Ok;
use dotenv::dotenv;
use std::env;

use tradingview::{
    Interval, chart::ChartOptions, handler::message::TradingViewCommand,
    pine_indicator::ScriptType, socket::DataServer, websocket::WebSocketClient,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    tracing_subscriber::fmt::init();
    let auth_token = env::var("TV_AUTH_TOKEN").expect("TV_AUTH_TOKEN is not set");

    let (response_tx, mut response_rx) = tokio::sync::mpsc::unbounded_channel();
    let (command_tx, command_rx) = tokio::sync::mpsc::unbounded_channel();

    let websocket = WebSocketClient::builder()
        .server(DataServer::ProData)
        .auth_token(&auth_token)
        .command_rx(command_rx)
        .response_tx(response_tx)
        .build()
        .await
        .unwrap();

    // let opts = ChartOptions::new_with("BTCUSDT", "BINANCE", Interval::OneMinute).bar_count(100);
    // let opts2 = ChartOptions::new_with("BTCUSDT", "BINANCE", Interval::OneDay)
    //     .bar_count(1)
    //     .study_config(
    //         "STD;Candlestick%1Pattern%1Bearish%1Abandoned%1Baby",
    //         "33.0",
    //         ScriptType::IntervalScript,
    //     );
    // let opts3 = ChartOptions::new_with("BTCUSDT", "BINANCE", Interval::OneHour)
    //     .replay_mode(true)
    //     .replay_from(1698624060);

    // websocket
    //     .set_market(opts)
    //     .await?
    //     .set_market(opts2)
    //     .await?
    //     .set_market(opts3)
    //     .await?;

    // websocket
    //     .create_quote_session()
    //     .await?
    //     .set_fields()
    //     .await?
    //     .add_symbols(&[
    //         // "SP:SPX",
    //         "BINANCE:BTCUSDT",
    //         // "BINANCE:ETHUSDT",
    //         // "BITSTAMP:ETHUSD",
    //         // "NASDAQ:TSLA", // "BINANCE:B",
    //     ])
    //     .await?;

    // Spawn WebSocket tasks in background
    let websocket_handle = tokio::spawn(async move {
        // Run both command handling and subscription concurrently
        let command_task = websocket.handle_commands();
        tracing::info!("WebSocket command handler started");
        let subscribe_task = websocket.subscribe();
        tracing::info!("WebSocket subscription started");

        // Use select to run both concurrently and handle either completing
        tokio::select! {
            result = command_task => {
                tracing::info!("WebSocket command handler completed");
                if let Err(e) = result {
                    tracing::error!("Command handler error: {}", e);
                }
            }
            _ = subscribe_task => {
                tracing::info!("WebSocket subscription completed");
            }
        }
    });

    tokio::spawn(async move {
        loop {
            match response_rx.recv().await {
                Some(res) => match res {
                    tradingview::handler::message::TradingViewResponse::ChartData(
                        series_info,
                        data_points,
                    ) => {
                        tracing::trace!(
                            "Chart Data: Series Info: {:?}, Data Points: {:?}",
                            series_info,
                            data_points
                        );
                    }
                    tradingview::handler::message::TradingViewResponse::QuoteData(quote_value) => {
                        tracing::info!("Quote Data: {:?}", quote_value);
                    }
                    tradingview::handler::message::TradingViewResponse::StudyData(
                        study_options,
                        study_response_data,
                    ) => {
                        tracing::trace!(
                            "Study Data: Options: {:?}, Response Data: {:?}",
                            study_options,
                            study_response_data
                        );
                    }
                    tradingview::handler::message::TradingViewResponse::Error(error, values) => {
                        tracing::error!("Error: {:?}, Values: {:?}", error, values);
                    }
                    tradingview::handler::message::TradingViewResponse::SymbolInfo(symbol_info) => {
                        tracing::trace!("Symbol Info: {:?}", symbol_info);
                    }
                    tradingview::handler::message::TradingViewResponse::SeriesCompleted(values) => {
                        tracing::trace!("Series Completed: {:?}", values);
                    }
                    tradingview::handler::message::TradingViewResponse::SeriesLoading(
                        loading_msg,
                    ) => {
                        tracing::trace!("Series Loading: {:?}", loading_msg);
                    }
                    tradingview::handler::message::TradingViewResponse::QuoteCompleted(values) => {
                        tracing::trace!("Quote Completed: {:?}", values);
                    }
                    tradingview::handler::message::TradingViewResponse::ReplayOk(values) => {
                        tracing::trace!("Replay OK: {:?}", values);
                    }
                    tradingview::handler::message::TradingViewResponse::ReplayPoint(values) => {
                        tracing::trace!("Replay Point: {:?}", values);
                    }
                    tradingview::handler::message::TradingViewResponse::ReplayInstanceId(
                        values,
                    ) => {
                        tracing::trace!("Replay Instance ID: {:?}", values);
                    }
                    tradingview::handler::message::TradingViewResponse::ReplayResolutions(
                        values,
                    ) => {
                        tracing::trace!("Replay Resolutions: {:?}", values);
                    }
                    tradingview::handler::message::TradingViewResponse::ReplayDataEnd(values) => {
                        tracing::trace!("Replay Data End: {:?}", values);
                    }
                    tradingview::handler::message::TradingViewResponse::StudyLoading(
                        loading_msg,
                    ) => {
                        tracing::trace!("Study Loading: {:?}", loading_msg);
                    }
                    tradingview::handler::message::TradingViewResponse::StudyCompleted(values) => {
                        tracing::trace!("Study Completed: {:?}", values);
                    }
                    tradingview::handler::message::TradingViewResponse::UnknownEvent(
                        ustr,
                        values,
                    ) => {
                        tracing::warn!("Unknown Event: {:?}, Values: {:?}", ustr, values);
                    }
                },
                None => {
                    eprintln!("Response channel closed");
                }
            };
        }
    });

    command_tx.send(TradingViewCommand::CreateQuoteSession)?;
    command_tx.send(TradingViewCommand::SetQuoteFields)?;
    command_tx.send(TradingViewCommand::QuoteFastSymbols {
        symbols: vec![
            "BINANCE:BTCUSDT".into(),
            // "BINANCE:ETHUSDT".into(),
            // "BITSTAMP:ETHUSD".into(),
            // "NASDAQ:TSLA".into(),
        ],
    })?;

    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(90)).await;
        command_tx.send(TradingViewCommand::QuoteFastSymbols {
            symbols: vec![
                // "BINANCE:BTCUSDT".into(),
                // "BINANCE:ETHUSDT".into(),
                // "BITSTAMP:ETHUSD".into(),
                "NASDAQ:TSLA".into(),
            ],
        })?;
    }

    Ok(())
}
