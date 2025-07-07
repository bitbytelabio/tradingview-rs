use dotenv::dotenv;
use std::{env, sync::Arc};

use tradingview::{
    Interval, chart::ChartOptions, handler::message::Command, pine_indicator::ScriptType,
    socket::DataServer, websocket::WebSocketClient,
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
        .data_tx(response_tx)
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
    //         "SP:SPX",
    //         "BINANCE:BTCUSDT",
    //         "BINANCE:ETHUSDT",
    //         "BITSTAMP:ETHUSD",
    //         "NASDAQ:TSLA", // "BINANCE:B",
    //     ])
    //     .await?;

    tokio::spawn(async move {
        websocket.subscribe().await;
        while let Some(res) = response_rx.recv().await {
            match res {
                tradingview::handler::message::TradingViewResponse::ChartData(
                    series_info,
                    data_points,
                ) => {
                    println!(
                        "Chart Data: Series Info: {:?}, Data Points: {:?}",
                        series_info, data_points
                    );
                }
                tradingview::handler::message::TradingViewResponse::QuoteData(quote_value) => {
                    println!("Quote Data: {:?}", quote_value);
                }
                tradingview::handler::message::TradingViewResponse::StudyData(
                    study_options,
                    study_response_data,
                ) => {
                    println!(
                        "Study Data: Options: {:?}, Response Data: {:?}",
                        study_options, study_response_data
                    );
                }
                tradingview::handler::message::TradingViewResponse::Error(error, values) => {
                    eprintln!("Error: {:?}, Values: {:?}", error, values);
                }
                tradingview::handler::message::TradingViewResponse::SymbolInfo(symbol_info) => {
                    println!("Symbol Info: {:?}", symbol_info);
                }
                tradingview::handler::message::TradingViewResponse::SeriesCompleted(values) => {
                    println!("Series Completed: {:?}", values);
                }
                tradingview::handler::message::TradingViewResponse::SeriesLoading(loading_msg) => {
                    println!("Series Loading: {:?}", loading_msg);
                }
                tradingview::handler::message::TradingViewResponse::QuoteCompleted(values) => {
                    println!("Quote Completed: {:?}", values);
                }
                tradingview::handler::message::TradingViewResponse::ReplayOk(values) => {
                    println!("Replay OK: {:?}", values);
                }
                tradingview::handler::message::TradingViewResponse::ReplayPoint(values) => {
                    println!("Replay Point: {:?}", values);
                }
                tradingview::handler::message::TradingViewResponse::ReplayInstanceId(values) => {
                    println!("Replay Instance ID: {:?}", values);
                }
                tradingview::handler::message::TradingViewResponse::ReplayResolutions(values) => {
                    println!("Replay Resolutions: {:?}", values);
                }
                tradingview::handler::message::TradingViewResponse::ReplayDataEnd(values) => {
                    println!("Replay Data End: {:?}", values);
                }
                tradingview::handler::message::TradingViewResponse::StudyLoading(loading_msg) => {
                    println!("Study Loading: {:?}", loading_msg);
                }
                tradingview::handler::message::TradingViewResponse::StudyCompleted(values) => {
                    println!("Study Completed: {:?}", values);
                }
                tradingview::handler::message::TradingViewResponse::UnknownEvent(ustr, values) => {
                    println!("Unknown Event: {:?}, Values: {:?}", ustr, values);
                }
            };
        }
    });

    command_tx.send(Command::CreateQuoteSession)?;
    command_tx.send(Command::SetQuoteFields)?;
    command_tx.send(Command::QuoteFastSymbols {
        symbols: vec![
            "BINANCE:BTCUSDT".into(),
            // "BINANCE:ETHUSDT".into(),
            // "BITSTAMP:ETHUSD".into(),
            // "NASDAQ:TSLA".into(),
        ],
    })?;

    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(90)).await;
        command_tx.send(Command::QuoteFastSymbols {
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
