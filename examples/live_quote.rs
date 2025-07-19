use std::{env, sync::Arc};
use tokio::sync::mpsc;
use tradingview::{
    DataServer,
    live::handler::{
        Handler,
        command::{Command, CommandRunner},
        message::{CommandMsg, QuoteCommandMsg},
    },
    utils::gen_session_id,
    websocket::WebSocketClient,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();

    // Use debug level to see more details
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    // Create WebSocket client
    let ws_client = WebSocketClient::builder()
        .server(DataServer::Data)
        .handler(Handler::default())
        .build()
        .await?;

    // let auth_token = env::var("TV_AUTH_TOKEN").expect("TV_AUTH_TOKEN is not set");
    let (command_tx, command_rx) = mpsc::unbounded_channel();
    let command_runner = CommandRunner::new(command_rx, Arc::clone(&ws_client));
    tokio::spawn(async move {
        if let Err(e) = command_runner.run().await {
            tracing::error!("Command runner error: {:?}", e);
        }
    });

    {
        use Command::*;
        let quote_session = gen_session_id("qs");
        command_tx
            .send(CreateQuoteSession(
                CommandMsg::builder().inner(&quote_session).build(),
            ))
            .unwrap();
        command_tx
            .send(SetQuoteFields(
                CommandMsg::builder().inner(&quote_session).build(),
            ))
            .unwrap();
        command_tx
            .send(AddQuoteSymbols(
                QuoteCommandMsg::builder()
                    .quote_session(&quote_session)
                    .symbols(vec!["OKX:BTCUSDT.P".into(), "BINANCE:ETHUSDT".into()])
                    .build(),
            ))
            .unwrap();
    }

    loop {}
    Ok(())
}
