use std::env;

use tradingview_rs::user::User;

use tracing::error;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let session = env::var("TV_SESSION").unwrap();
    let signature = env::var("TV_SIGNATURE").unwrap();

    let user = User::build()
        .session(&session, &signature)
        .get()
        .await
        .unwrap();

    // let client = Client::new(user);

    // let user_clone = user;
    // let search_type = Arc::new("".to_owned());

    match tradingview_rs::client::list_symbols(&user, None).await {
        Ok(symbols) => {
            println!("{:#?}", symbols.len());
        }
        Err(e) => {
            error!("{:#?}", e);
        }
    }

    // println!("{:#?}", symbols.len());

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

    // let indicators = client.get_builtin_indicators().await.unwrap();
    // for indicator in indicators {
    //     let resp = client.get_indicator_metadata(&indicator).await;

    //     match resp {
    //         Ok(resp) => {
    //             println!("{:#?}", resp);
    //         }
    //         Err(e) => {
    //             error!("{:#?}", e);
    //         }
    //     }
    // }
}
