#[cfg(test)]
mod tests {
    use std::env;
    use tradingview_rs::client::mics::*;
    use tradingview_rs::models::*;
    use tradingview_rs::user::*;

    #[tokio::test]
    async fn test_get_builtin_indicators() {
        let session = env::var("TV_SESSION").unwrap();
        let signature = env::var("TV_SIGNATURE").unwrap();

        let user = User::build()
            .session(&session, &signature)
            .get()
            .await
            .unwrap();

        let indicators = get_builtin_indicators(&user, BuiltinIndicators::All).await;
        assert!(indicators.is_ok());
        assert!(indicators.unwrap().len() > 0);
    }
}
