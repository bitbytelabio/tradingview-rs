use tracing::{debug, info};

mod auth;
mod misc_requests;
mod utils;

pub static UA: &'static str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/112.0.0.0 Safari/537.36 uacq";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    // info!("Starting up");
    // let indicators = misc_requests::get_builtin_indicators().await?;
    // debug!("Indicators: {:?}", indicators);

    println!("{}", utils::client::gen_session_id("qs"));
    let user = auth::get_user(
        "wow63q1l614ilkutj8tkc7zwhp87e09b",
        "v1:I6szYEiR40S888deJb6fJ33fnOVLGw1JbhP+7Hw63+U=",
        None,
    )
    .await
    .unwrap();

    // let user = auth::login_user("batttheyshool0211", "batttheyshool0211").await?;
    info!("User: {:#?}", user);
    Ok(())
}
