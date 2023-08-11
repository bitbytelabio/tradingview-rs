use std::env;

use tradingview_rs::user::User;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let session = env::var("TV_SESSION").unwrap();
    let signature = env::var("TV_SIGNATURE").unwrap();

    let _user = User::build()
        .session(&session, &signature)
        .get()
        .await
        .unwrap();

    // let mut socket = ChartSocket::new(DataServer::Data)
    //     .auth_token(user.auth_token)
    //     .build()
    //     .await
    //     .unwrap();

    // socket.event_loop().await;
}
