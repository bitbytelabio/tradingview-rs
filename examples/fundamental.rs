use dotenv::dotenv;
use tradingview::client::fundamental::test01;
#[tokio::main]
async fn main() {
    dotenv().ok();
    tracing_subscriber::fmt::init();
    test01().await.unwrap();
}
