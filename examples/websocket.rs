use futures_util::{SinkExt, StreamExt};
use tokio::io::{AsyncWriteExt, Result};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

#[tokio::main]
pub async fn main() -> Result<()> {
    println!("Hello, tokio-tungstenite!");

    let url = url::Url::parse("wss://ws.kraken.com").unwrap();

    let (ws_stream, _response) = connect_async(url).await.expect("Failed to connect");
    println!("WebSocket handshake has been successfully completed");

    let (mut write, read) = ws_stream.split();

    println!("sending");

    write
        .send(Message::Text(
            r#"{
        "event": "ping",
        "reqid": 42
      }"#
            .to_string()
                + "\n",
        ))
        .await
        .unwrap();

    println!("sent");

    let read_future = read.for_each(|message| async {
        println!("receiving...");
        let data = message.unwrap().into_data();
        tokio::io::stdout().write(&data).await.unwrap();
        println!("received...");
    });

    read_future.await;

    Ok(())
}
