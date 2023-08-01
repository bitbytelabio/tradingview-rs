#[cfg(test)]
mod user {
    use std::env;
    use tradingview_rs::error::*;
    use tradingview_rs::user::*;

    #[tokio::test]
    async fn test_login_empty_credentials() {
        let mut empty_user = User::default();
        empty_user.auth_token = "unauthorized_user_token".to_string();

        let result = User::new().credentials("", "").build().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), empty_user);
    }

    #[tokio::test]
    async fn test_login_with_credentials() {
        let mut empty_user = User::default();
        empty_user.auth_token = "unauthorized_user_token".to_string();

        let username = env::var("TV_USERNAME").unwrap();
        let password = env::var("TV_PASSWORD").unwrap();

        let result = User::new().credentials(&username, &password).build().await;

        assert!(result.is_ok());
        let user = result.unwrap();
        assert_ne!(user, empty_user);
        assert!(!user.auth_token.is_empty());
        assert!(!user.session.is_empty());
        assert!(!user.signature.is_empty());
        assert!(!user.session_hash.is_empty());
        assert!(!user.private_channel.is_empty());
        assert!(!user.auth_token.is_empty());
    }

    #[tokio::test]
    async fn test_login_with_invalid_credentials() {
        let mut empty_user = User::default();
        empty_user.auth_token = "unauthorized_user_token".to_string();

        let result = User::new().credentials("invalid", "invalid").build().await;

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(matches!(
            error,
            tradingview_rs::error::Error::LoginError(LoginError::InvalidCredentials)
        ));
    }

    #[tokio::test]
    async fn test_login_with_credentials_and_mfa() {
        let mut empty_user = User::default();
        empty_user.auth_token = "unauthorized_user_token".to_string();

        let username = env::var("TV_TOTP_USERNAME").unwrap();
        let password = env::var("TV_TOTP_PASSWORD").unwrap();
        let totp = env::var("TV_TOTP_SECRET").unwrap();

        let result = User::new()
            .credentials(&username, &password)
            .totp_secret(&totp)
            .build()
            .await;

        assert!(result.is_ok());
        let user = result.unwrap();
        assert_ne!(user, empty_user);
        assert!(!user.auth_token.is_empty());
        assert!(!user.session.is_empty());
        assert!(!user.signature.is_empty());
        assert!(!user.session_hash.is_empty());
        assert!(!user.private_channel.is_empty());
        assert!(!user.auth_token.is_empty());
    }

    #[tokio::test]
    async fn test_user_with_valid_session() {
        let mut empty_user = User::default();
        empty_user.auth_token = "unauthorized_user_token".to_string();

        let session = env::var("TV_SESSION").unwrap();
        let signature = env::var("TV_SIGNATURE").unwrap();

        let result = User::new().session(&session, &signature).build().await;

        assert!(result.is_ok());
        let user = result.unwrap();
        assert_ne!(user, empty_user);
        assert!(!user.auth_token.is_empty());
        assert!(!user.session.is_empty());
        assert!(!user.signature.is_empty());
        assert!(!user.session_hash.is_empty());
        assert!(!user.private_channel.is_empty());
        assert!(!user.auth_token.is_empty());
    }

    #[tokio::test]
    async fn test_user_with_invalid_session() {
        let mut empty_user = User::default();
        empty_user.auth_token = "unauthorized_user_token".to_string();

        let result = User::new()
            .session("invalid_session", "invalid_session_signature")
            .build()
            .await;

        assert!(result.is_err());
        let error = result.unwrap_err();

        assert!(matches!(
            error,
            tradingview_rs::error::Error::LoginError(LoginError::InvalidSession)
        ));
    }

    #[tokio::test]
    async fn test_user_with_credentials_and_invalid_mfa() {
        let mut empty_user = User::default();
        empty_user.auth_token = "unauthorized_user_token".to_string();

        let username = env::var("TV_TOTP_USERNAME").unwrap();
        let password = env::var("TV_TOTP_PASSWORD").unwrap();

        let result = User::new()
            .credentials(&username, &password)
            .totp_secret("ZTIXV4KTRISK4KK7")
            .build()
            .await;

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(matches!(
            error,
            tradingview_rs::error::Error::LoginError(LoginError::InvalidOTPSecret)
        ));
    }
}
