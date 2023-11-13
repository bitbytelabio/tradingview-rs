use dotenv::dotenv;

use tracing::info;
use tradingview::models::UserCookies;

#[tokio::main]
async fn main() {
    dotenv().ok();
    tracing_subscriber::fmt::init();

    let user = UserCookies::default().login("", "", None).await.unwrap();

    info!("User: {:?}", user.auth_token);
}
