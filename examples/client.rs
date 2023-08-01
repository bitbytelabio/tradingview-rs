use std::env;
use tradingview_rs::{client::Client, user::User};

use tradingview_rs::error::Error;
type Result<T> = std::result::Result<T, Error>;
use tracing::error;

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

    // let symbols = client.list_symbols(None).await.unwrap();
    // println!("{:#?}", symbols);

    // let chart_token = client.get_chart_token("jUwT1z48").await.unwrap();
    // print!("{:#?}", chart_token);

    // client.get_ta("HOSE", &["FPT", "HVN", "VNM"]).await;
    // let rsp = client.search_symbol().await;
    // println!("{:#?}", rsp);

    // let resp = client
    //     .get_drawing("jUwT1z48", "NASDAQ:AAPL", "_shared")
    //     .await
    //     .unwrap();

    // println!("{:#?}", resp);

    let indicators = client.get_builtin_indicators().await.unwrap();
    for indicator in indicators {
        let resp = client.get_indicator_metadata(&indicator).await;

        match resp {
            Ok(resp) => {
                println!("{:#?}", resp);
            }
            Err(e) => {
                error!("{:#?}", e);
            }
        }
    }
}
