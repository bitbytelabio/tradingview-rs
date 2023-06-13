#[cfg(test)]
mod auth {
    use std::env;
    use tradingview_rs::auth::*;

    #[tokio::test]
    async fn get_user_test() {
        match get_user("invalid_session", "invalid_signature", None).await {
            Ok(_) => assert!(false, "Wrong session and signature should not return user"),
            Err(err) => assert!(err
                .to_string()
                .contains("Wrong or expired sessionid/signature")),
        }
    }

    #[tokio::test]
    async fn get_user_with_valid_session() {
        let session = env::var("TV_SESSION").unwrap();
        let signature = env::var("TV_SIGNATURE").unwrap();
        match get_user(&session, &signature, None).await {
            Ok(user) => assert!(
                || -> bool {
                    user.id > 0
                        && user.username.len() > 0
                        && user.session.len() > 0
                        && user.signature.len() > 0
                        && user.session_hash.len() > 0
                        && user.private_channel.len() > 0
                        && user.auth_token.len() > 0
                }(),
                "Cannot get user with valid session"
            ),

            Err(_) => assert!(false, "Cannot get user with valid session"),
        }
    }
}
