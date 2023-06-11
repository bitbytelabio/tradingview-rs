pub mod utils;

use tracing::{debug, info};

mod misc_requests;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    info!("Starting up");
    let indicators = misc_requests::get_builtin_indicators().await?;
    debug!("Indicators: {:?}", indicators.len());
    println!("{}", crate::utils::client::gen_session_id("qs"));
    Ok(())
}
