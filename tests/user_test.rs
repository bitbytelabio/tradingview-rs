// #[cfg(test)]
// mod tests {
//     use dotenv::dotenv;
//     use std::env;
//     use tradingview::error::*;
//     use tradingview::user::*;

//     #[tokio::test]
//     #[ignore]
//     async fn test_login_empty_credentials() {
//         let mut empty_user = User::default();
//         empty_user.auth_token = "unauthorized_user_token".to_string();

//         let result = User::build().credentials("", "").get().await;
//         assert!(result.is_ok());
//         assert_eq!(result.unwrap(), empty_user);
//     }

//     #[tokio::test]
//     #[ignore]
//     async fn test_login_with_credentials() {
//         dotenv().ok();
//         let mut empty_user = User::default();
//         empty_user.auth_token = "unauthorized_user_token".to_string();

//         let username = env::var("TV_USERNAME").unwrap();
//         let password = env::var("TV_PASSWORD").unwrap();

//         let result = User::build().credentials(&username, &password).get().await;

//         assert!(result.is_ok());
//         let user = result.unwrap();
//         assert_ne!(user, empty_user);
//         assert!(!user.auth_token.is_empty());
//         assert!(!user.session.is_empty());
//         assert!(!user.signature.is_empty());
//         assert!(!user.session_hash.is_empty());
//         assert!(!user.private_channel.is_empty());
//         assert!(!user.auth_token.is_empty());
//     }

//     #[tokio::test]
//     #[ignore]
//     async fn test_login_with_invalid_credentials() {
//         let mut empty_user = User::default();
//         empty_user.auth_token = "unauthorized_user_token".to_string();

//         let result = User::build().credentials("invalid", "invalid").get().await;

//         assert!(result.is_err());
//         let error = result.unwrap_err();
//         assert!(
//             matches!(error, tradingview::error::Error::LoginError(LoginError::InvalidCredentials))
//         );
//     }

//     #[tokio::test]
//     #[ignore]
//     async fn test_login_with_credentials_and_mfa() {
//         dotenv().ok();
//         let mut empty_user = User::default();
//         empty_user.auth_token = "unauthorized_user_token".to_string();

//         let username = env::var("TV_TOTP_USERNAME").unwrap();
//         let password = env::var("TV_TOTP_PASSWORD").unwrap();
//         let totp = env::var("TV_TOTP_SECRET").unwrap();

//         let result = User::build().credentials(&username, &password).totp_secret(&totp).get().await;

//         assert!(result.is_ok());
//         let user = result.unwrap();
//         assert_ne!(user, empty_user);
//         assert!(!user.auth_token.is_empty());
//         assert!(!user.session.is_empty());
//         assert!(!user.signature.is_empty());
//         assert!(!user.session_hash.is_empty());
//         assert!(!user.private_channel.is_empty());
//         assert!(!user.auth_token.is_empty());
//     }

//     #[tokio::test]
//     #[ignore]
//     async fn test_user_with_valid_session() {
//         dotenv().ok();
//         let mut empty_user = User::default();
//         empty_user.auth_token = "unauthorized_user_token".to_string();

//         let session = env::var("TV_SESSION").unwrap();
//         let signature = env::var("TV_SIGNATURE").unwrap();

//         let result = User::build().session(&session, &signature).get().await;

//         assert!(result.is_ok());
//         let user = result.unwrap();
//         assert_ne!(user, empty_user);
//         assert!(!user.auth_token.is_empty());
//         assert!(!user.session.is_empty());
//         assert!(!user.signature.is_empty());
//         assert!(!user.session_hash.is_empty());
//         assert!(!user.private_channel.is_empty());
//         assert!(!user.auth_token.is_empty());
//     }

//     #[tokio::test]
//     #[ignore]
//     async fn test_user_with_invalid_session() {
//         let mut empty_user = User::default();
//         empty_user.auth_token = "unauthorized_user_token".to_string();

//         let result = User::build()
//             .session("invalid_session", "invalid_session_signature")
//             .get().await;

//         assert!(result.is_err());
//         let error = result.unwrap_err();

//         assert!(matches!(error, tradingview::error::Error::LoginError(LoginError::InvalidSession)));
//     }

//     #[tokio::test]
//     #[ignore]
//     async fn test_user_with_credentials_and_invalid_mfa() {
//         dotenv().ok();
//         let mut empty_user = User::default();
//         empty_user.auth_token = "unauthorized_user_token".to_string();

//         let username = env::var("TV_TOTP_USERNAME").unwrap();
//         let password = env::var("TV_TOTP_PASSWORD").unwrap();

//         let result = User::build()
//             .credentials(&username, &password)
//             .totp_secret("ZTIXV4KTRISK4KK7")
//             .get().await;

//         assert!(result.is_err());
//         let error = result.unwrap_err();
//         assert!(
//             matches!(error, tradingview::error::Error::LoginError(LoginError::InvalidOTPSecret))
//         );
//     }
// }
