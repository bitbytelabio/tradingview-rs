#![cfg(feature = "user")]
use std::fs::File;
use std::io::Write;
use tradingview::UserCookies;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv::dotenv().ok();
    let username = std::env::var("TV_USERNAME").expect("TV_USERNAME is not set");
    let password = std::env::var("TV_PASSWORD").expect("TV_PASSWORD is not set");
    let totp = std::env::var("TV_TOTP_SECRET").expect("TV_TOTP_SECRET is not set");

    tracing_subscriber::fmt::init();
    let user = UserCookies::default()
        .login(&username, &password, Some(&totp))
        .await?;

    tracing::info!("User: {:?}", user);

    // Serialize the user cookies to JSON
    let json = serde_json::to_string_pretty(&user)?;

    // Create a file with a meaningful name
    let filepath = "tv_user_cookies.json";
    let mut file = File::create(filepath)?;

    // Write the JSON to the file
    file.write_all(json.as_bytes())?;

    tracing::info!("User cookies saved to {}", filepath);

    Ok(())
}
