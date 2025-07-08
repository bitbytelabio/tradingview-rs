use dotenv::dotenv;
use std::{env, sync::Arc};

use tradingview::live::{
    handler::message::{Command, CommandHandler, TradingViewResponse},
    socket::DataServer,
    websocket::WebSocketClient,
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
        .await?;

    let websocket = Arc::new(websocket);

    let shared_websocket = websocket.clone();

    tokio::spawn(async move {
        shared_websocket.subscribe().await;

        while let Some(res) = response_rx.recv().await {
            match res {
                TradingViewResponse::QuoteData(quote_value) => {
                    println!("Quote Data: {quote_value:?}");
                }

                TradingViewResponse::Error(error, values) => {
                    eprintln!("Error: {error:?}, Values: {values:?}");
                }
                _ => {
                    println!("Received response: {res:?}");
                }
            };
        }
    });

    let mut command_handler = CommandHandler::new(command_rx, websocket.clone());
    tokio::spawn(async move {
        if let Err(e) = command_handler.handle_commands().await {
            tracing::error!("Error handling commands: {:?}", e);
        }
    });

    command_tx.send(Command::CreateQuoteSession)?;
    command_tx.send(Command::SetQuoteFields)?;
    command_tx.send(Command::QuoteFastSymbols {
        symbols: vec!["BINANCE:BTCUSDT".into()],
    })?;

    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        command_tx.send(Command::QuoteRemoveSymbols {
            symbols: vec!["BINANCE:BTCUSDT".into()],
        })?;

        command_tx.send(Command::QuoteRemoveSymbols {
            symbols: vec!["BINANCE:BTCUSDT".into()],
        })?;

        // Simulate some other commands
        command_tx.send(Command::Delete).unwrap();
    }
}
