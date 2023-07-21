use tradingview_rs::user::User;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let mut user = User::new(
        Some("lite@bitbytelab.io".to_string()),
        Some("dAIuLpdzmEy8HWnIYRGwigRA4XwJT4Ny/WIsD/rXy5qurJwu".to_string()),
        Some("PTB2JVFN3YXVGVFX".to_string()),
    )
    .await;

    // let user = User::get_user(
    //     "ztpez0vb32w1zdu3yzlhc5egqlo67yee".to_owned(),
    //     "v1:8fRMIzAGzeK9ufTNs7L7B0ZeplG2inabghj2JSuBg4g=".to_owned(),
    //     None,
    // )
    // .await
    // .unwrap();
    println!("User1: {:#?}", user);

    user.update_token().await.unwrap();

    println!("User2: {:#?}", user);
}
