use futures_util::{future, pin_mut, SinkExt, StreamExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, protocol::Message},
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
    let (ws_stream, res) = connect_async(request).await.unwrap();
    // Log the response
    tracing::debug!("response: {:?}", res);
    println!("WebSocket handshake has been successfully completed");

    let (mut write, read) = ws_stream.split();

    let my_messsages: Vec<tungstenite::Message> = vec![
        tungstenite::Message::Text(
            "~m~54~m~{\"m\":\"set_auth_token\",\"p\":[\"unauthorized_user_token\"]}".to_string(),
        ),
        tungstenite::Message::Text(
            "~m~34~m~{\"m\":\"set_locale\",\"p\":[\"en\",\"US\"]}".to_string(),
        ),
    ];

    for msg in my_messsages {
        write.send(msg).await.unwrap(); // Send each message with a 5 second interval
    }
    let ws_to_stdout = {
        read.for_each(|message| async {
            let data = message.unwrap().into_data();
            tokio::io::stdout().write_all(&data).await.unwrap();
        })
    };
    ws_to_stdout.await;
}
