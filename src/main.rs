use tradingview_rs::user::User;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let mut user1 = User::new(
        Some("lite_bitbytelab".to_string()),
        Some("dAIuLpdzmEy8HWnIYRGwigRA4XwJT4Ny/WIsD/rXy5qurJwu".to_string()),
        Some("PTB2JVFN3YXVGVFX".to_owned()),
    )
    .await;
    println!("User1: {:#?}", user1);

    let user2: User = match User::get_user(
        user1.session.clone(),
        user1.session_signature.clone(),
        Some(user1.is_pro.clone()),
        Some("https://www.tradingview.com/markets/".to_string()),
    )
    .await
    {
        Ok(user) => user,
        Err(e) => {
            println!("Error: {}", e);
            return;
        }
    };

    // user.update_token().await.unwrap();

    println!("User2: {:#?}", user2);
}
