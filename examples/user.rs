use dotenv::dotenv;
use std::env;
use tracing::info;
use tradingview::user::User;

#[tokio::main]
async fn main() {
    dotenv().ok();
    tracing_subscriber::fmt::init();

    let session = env::var("TV_SESSION").unwrap();
    let signature = env::var("TV_SIGNATURE").unwrap();

    let user = User::build()
        .session(&session, &signature)
        .get()
        .await
        .unwrap();

    info!("User: {:?}", user.auth_token);
}
