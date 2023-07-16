#[cfg(test)]
mod auth {
    use std::env;

    #[tokio::test]
    async fn get_user_test_with_invalid_session() {
        // tracing_subscriber::fmt::init();
        match tradingview_rs::auth::get_user("invalid_session", "invalid_signature", None).await {
            Ok(_) => assert!(false, "Wrong session and signature should not return user"),
            Err(err) => assert!(err
                .to_string()
                .contains("Wrong or expired sessionid/signature")),
        }
    }

    #[tokio::test]
    async fn get_user_with_valid_session() {
        // tracing_subscriber::fmt::init();
        let session = env::var("TV_SESSION").unwrap();
        let signature = env::var("TV_SIGNATURE").unwrap();
        match tradingview_rs::auth::get_user(&session, &signature, None).await {
            Ok(user) => assert!(
                || -> bool {
                    user.id > 0
                        && !user.username.is_empty()
                        && !user.session.is_empty()
                        && !user.signature.is_empty()
                        && !user.session_hash.is_empty()
                        && !user.private_channel.is_empty()
                        && !user.auth_token.is_empty()
                }(),
                "Cannot get user data with valid session"
            ),

            Err(err) => assert!(
                false,
                "Cannot get user data with valid session, unwrap data error: {:#?}",
                err
            ),
        }
    }

    #[tokio::test]
    async fn login_user_with_valid_credential() {
        // tracing_subscriber::fmt::init();
        let username = env::var("TV_USERNAME").unwrap();
        let password = env::var("TV_PASSWORD").unwrap();
        match tradingview_rs::auth::login_user(&username, &password, None).await {
            Ok(user) => assert!(
                || -> bool {
                    user.id > 0
                        && !user.username.is_empty()
                        && !user.session.is_empty()
                        && !user.signature.is_empty()
                        && !user.session_hash.is_empty()
                        && !user.private_channel.is_empty()
                        && !user.auth_token.is_empty()
                }(),
                "Cannot login user with valid credential"
            ),

            Err(err) => assert!(
                false,
                "Cannot login user with valid credential, unwrap data error: {}",
                err.to_string()
            ),
        }
    }

    #[tokio::test]
    async fn login_user_with_invalid_credential() {
        // tracing_subscriber::fmt::init();
        match tradingview_rs::auth::login_user("invalid_username", "invalid_password", None).await {
            Ok(_) => assert!(false, "Wrong username and password should not return user"),
            Err(err) => assert!(err.to_string().contains("Wrong username or password")),
        }
    }

    #[tokio::test]
    async fn login_user_with_valid_totp() {
        // tracing_subscriber::fmt::init();
        let username = env::var("TV_TOTP_USERNAME").unwrap();
        let password = env::var("TV_TOTP_PASSWORD").unwrap();
        let totp = env::var("TV_TOTP_SECRET").unwrap();
        match tradingview_rs::auth::login_user(&username, &password, Some(totp)).await {
            Ok(user) => assert!(
                || -> bool {
                    user.id > 0
                        && !user.username.is_empty()
                        && !user.session.is_empty()
                        && !user.signature.is_empty()
                        && !user.session_hash.is_empty()
                        && !user.private_channel.is_empty()
                        && !user.auth_token.is_empty()
                }(),
                "Cannot login user with valid credential"
            ),

            Err(err) => assert!(
                false,
                "Cannot login user with valid credential, unwrap data error: {}",
                err.to_string()
            ),
        }
    }

    #[tokio::test]
    async fn login_user_with_invalid_totp() {
        // tracing_subscriber::fmt::init();
        let username = env::var("TV_TOTP_USERNAME").unwrap();
        let password = env::var("TV_TOTP_PASSWORD").unwrap();
        match tradingview_rs::auth::login_user(
            &username,
            &password,
            Some("ZTIXV4KTRISK4KK7".to_string()),
        )
        .await
        {
            Ok(_) => assert!(false, "Wrong totp should not return user"),
            Err(err) => assert!(
                err.to_string().contains("TOTP Secret is required")
                    || err.to_string().contains("Invalid TOTP secret")
                    || err.to_string().contains("Wrong username or password")
            ),
        }
    }
}
