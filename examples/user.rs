use tradingview::user::UserCookies;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let user = UserCookies::new()
        .login("testuser", "testpassword", None)
        .await?;

    tracing::info!("User: {:?}", user);
    Ok(())
}
