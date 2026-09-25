#![cfg(feature = "user")]
use anyhow::Context;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use tradingview::UserCookies;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    dotenv::dotenv().ok();

    let username = match std::env::var("TV_USERNAME") {
        Ok(val) => val,
        Err(std::env::VarError::NotPresent) => {
            anyhow::bail!("TV_USERNAME environment variable is not set");
        }
        Err(std::env::VarError::NotUnicode(_)) => {
            anyhow::bail!("TV_USERNAME environment variable contains invalid unicode");
        }
    };
    let password = match std::env::var("TV_PASSWORD") {
        Ok(val) => val,
        Err(std::env::VarError::NotPresent) => {
            anyhow::bail!("TV_PASSWORD environment variable is not set");
        }
        Err(std::env::VarError::NotUnicode(_)) => {
            anyhow::bail!("TV_PASSWORD environment variable contains invalid unicode");
        }
    };
    let totp = match std::env::var("TV_TOTP_SECRET") {
        Ok(val) => Some(val),
        Err(std::env::VarError::NotPresent) => None,
        Err(std::env::VarError::NotUnicode(_)) => {
            anyhow::bail!("TV_TOTP_SECRET environment variable contains invalid unicode");
        }
    };
    let captcha_api_key = match std::env::var("TWO_CAPTCHA_API_KEY") {
        Ok(key) if key.trim().is_empty() => {
            anyhow::bail!(
                "TWO_CAPTCHA_API_KEY is set but blank; provide a valid API key or unset the variable"
            );
        }
        Ok(key) => Some(key),
        Err(std::env::VarError::NotPresent) => None,
        Err(std::env::VarError::NotUnicode(_)) => {
            anyhow::bail!("TWO_CAPTCHA_API_KEY environment variable contains invalid unicode");
        }
    };

    let mut user_cookies = UserCookies::default();
    let user = match captcha_api_key {
        Some(api_key) => {
            tracing::info!(
                "TWO_CAPTCHA_API_KEY detected: CAPTCHA solving is enabled and may incur charges via 2Captcha if challenged"
            );
            user_cookies
                .login_with_captcha(&username, &password, totp.as_deref(), &api_key)
                .await?
        }
        None => {
            user_cookies
                .login(&username, &password, totp.as_deref())
                .await?
        }
    };
    tracing::info!("Logged in successfully");

    // Serialize the user cookies to JSON
    let json = serde_json::to_string_pretty(&user)?;

    let filepath = Path::new("tv_user_cookies.json");

    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }

    let mut file = options.open(filepath).with_context(|| {
        format!(
            "Refusing to overwrite existing file or failed to create private cookie file at '{}'",
            filepath.display()
        )
    })?;

    file.write_all(json.as_bytes())?;

    tracing::info!("User cookies saved to {}", filepath.display());
    Ok(())
}
