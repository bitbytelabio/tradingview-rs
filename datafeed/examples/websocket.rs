use futures_util::{future, pin_mut, StreamExt};
use std::env;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, protocol::Message},
};

#[tokio::main]
async fn main() {
    // Initialize tracing subscriber
    tracing_subscriber::fmt::init();
    // Get connection address from command line arguments
    let connect_addr = env::args()
        .nth(1)
        .unwrap_or_else(|| panic!("this program requires at least one argument"));
    // Parse the URL from the connection address
    let url = url::Url::parse(&connect_addr).unwrap();
    // Convert the URL into a client request
    let mut request = url.into_client_request().unwrap();
    // Add Origin header to the request
    let headers = request.headers_mut();
    headers.insert("Origin", "https://data.tradingview.com/".parse().unwrap());
    // Log the request
    tracing::debug!("request: {:?}", request);
    // Set up an unbounded channel for sending messages from stdin
    let (stdin_tx, stdin_rx) = futures_channel::mpsc::unbounded();
    // Spawn a task to read from stdin and send messages through the channel
    tokio::spawn(read_stdin(stdin_tx));
    // Connect to the WebSocket server
    let (ws_stream, _) = connect_async(request).await.expect("Failed to connect");
    println!("WebSocket handshake has been successfully completed");
    // Split the WebSocket stream into a write half and a read half
    let (write, read) = ws_stream.split();
    // Map incoming messages from stdin to WebSocket messages and send them
    let stdin_to_ws = stdin_rx.map(Ok).forward(write);
    // Map incoming WebSocket messages to stdout and print them
    let ws_to_stdout = {
        read.for_each(|message| async {
            let data = message.unwrap().into_data();
            tokio::io::stdout().write_all(&data).await.unwrap();
        })
    };
    // Wait for either stdin_to_ws or ws_to_stdout to complete
    pin_mut!(stdin_to_ws, ws_to_stdout);
    future::select(stdin_to_ws, ws_to_stdout).await;
}
// Helper method to read data from stdin and send it through the provided sender
async fn read_stdin(tx: futures_channel::mpsc::UnboundedSender<Message>) {
    let mut stdin = tokio::io::stdin();
    loop {
        let mut buf = vec![0; 1024];
        let n = match stdin.read(&mut buf).await {
            Err(_) | Ok(0) => break,
            Ok(n) => n,
        };
        buf.truncate(n);
        tx.unbounded_send(Message::binary(buf)).unwrap();
    }
}
