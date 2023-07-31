use std::env;
use tradingview_rs::{client::Client, user::User};

use tradingview_rs::error::Error;
type Result<T> = std::result::Result<T, Error>;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let session = env::var("TV_SESSION").unwrap();
    let signature = env::var("TV_SIGNATURE").unwrap();

    let user = User::new()
        .session(&session, &signature)
        .build()
        .await
        .unwrap();

    let client = Client::new(user);

    let symbols = client.list_symbols(None).await.unwrap();
    println!("{:#?}", symbols);

    // client.get_ta("HOSE", &["FPT", "HVN", "VNM"]).await;
    // let rsp = client.search_symbol().await;
    // println!("{:#?}", rsp);
}
