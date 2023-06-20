use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, connect, Message},
};

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

    socket.write_message(Message::Text(
        r#"{
    "action": "authenticate",
    "data": {
        "key_id": "API-KEY",
        "secret_key": "SECRET-KEY"
    }
}"#
        .into(),
    ));

    socket.write_message(Message::Text(
        r#"{
    "action": "listen",
    "data": {
        "streams": ["AM.SPY"]
    }
}"#
        .into(),
    ));

    loop {
        let msg = socket.read_message().expect("Error reading message");
        println!("Received: {}", msg);
    }
}
