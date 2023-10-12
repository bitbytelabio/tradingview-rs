use dotenv::dotenv;
use tradingview::client::collector::Collector;
#[tokio::main]
async fn main() {
    dotenv().ok();
    tracing_subscriber::fmt::init();
}
