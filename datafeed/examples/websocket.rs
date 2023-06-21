use tungstenite::{client::IntoClientRequest, connect, Message};

#[tokio::main]
async fn main() {
    // Initialize tracing subscriber
    tracing_subscriber::fmt::init();
    // Parse the URL from the connection address
    let url = url::Url::parse("wss://data.tradingview.com/socket.io/websocket").unwrap();
    // Convert the URL into a client request
    let mut request = url.into_client_request().unwrap();
    // Add Origin header to the request
    let headers = request.headers_mut();
    headers.insert("Origin", "https://data.tradingview.com/".parse().unwrap());
    // Log the request
    tracing::debug!("request: {:?}", request);
    // Connect to the WebSocket server
    let (mut socket, res) = connect(request).unwrap();
    // Log the response
    tracing::debug!("response: {:?}", res);
    println!("WebSocket handshake has been successfully completed");

    let _ = socket.write_message(Message::Text(
        r#"{
    "action": "authenticate",
    "data": {
        "key_id": "API-KEY",
        "secret_key": "SECRET-KEY"
    }
}"#
        .into(),
    ));

    let _ = socket.write_message(Message::Text(
        r#"{
    "action": "listen",
    "data": {
        "streams": ["AM.SPY"]
    }
}"#
        .into(),
    ));

    loop {
        let result = socket.read_message();
        match result {
            Ok(msg) => println!("Received: {}", msg),
            Err(e) => {
                println!("Error reading message: {:?}", e);
                break;
            }
        }
    }
}
