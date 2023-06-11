pub mod utils;

use tracing::{debug, info};

mod auth;
mod misc_requests;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    // info!("Starting up");
    let indicators = misc_requests::get_builtin_indicators().await?;
    debug!("Indicators: {:?}", indicators);

    // println!("{}", crate::utils::client::gen_session_id("qs"));
    // auth::get_user(
    //     "wow63q1l614ilkutj8tkc7zwhp87e09b",
    //     "v1:I6szYEiR40S888deJb6fJ33fnOVLGw1JbhP+7Hw63+U=",
    //     None,
    // )
    // .await;

    // auth::login_user("batttheyshool0211", "batttheyshool0211").await?;
    Ok(())
}
