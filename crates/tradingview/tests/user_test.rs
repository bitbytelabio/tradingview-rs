#![cfg(feature = "user")]

use tradingview::{
    error::{Error, LoginError},
    models::UserCookies,
    user::fetch_tradingview_token,
};

#[tokio::test]
async fn test_login_rejects_empty_credentials_offline() {
    let mut user = UserCookies::new();

    let err = user.login("", "", None).await.unwrap_err();
    assert!(
        matches!(
            err,
            Error::Login {
                source: LoginError::EmptyCredentials
            }
        ),
        "expected EmptyCredentials on both empty, got: {err:?}"
    );

    let err = user.login("  ", "  ", None).await.unwrap_err();
    assert!(
        matches!(
            err,
            Error::Login {
                source: LoginError::EmptyCredentials
            }
        ),
        "expected EmptyCredentials on whitespace, got: {err:?}"
    );

    let err = user.login("", "secret", None).await.unwrap_err();
    assert!(
        matches!(
            err,
            Error::Login {
                source: LoginError::EmptyCredentials
            }
        ),
        "expected EmptyCredentials on empty username, got: {err:?}"
    );

    let err = user.login("alice", "", None).await.unwrap_err();
    assert!(
        matches!(
            err,
            Error::Login {
                source: LoginError::EmptyCredentials
            }
        ),
        "expected EmptyCredentials on empty password, got: {err:?}"
    );
}

#[tokio::test]
async fn test_fetch_token_rejects_missing_session_offline() {
    let mut user = UserCookies::default();
    let err = fetch_tradingview_token(&user).await.unwrap_err();
    assert!(
        matches!(
            err,
            Error::Login {
                source: LoginError::SessionNotFound
            }
        ),
        "expected SessionNotFound on default user, got: {err:?}"
    );

    user.session = "valid_session".to_string();
    user.session_signature = "".to_string();
    let err = fetch_tradingview_token(&user).await.unwrap_err();
    assert!(
        matches!(
            err,
            Error::Login {
                source: LoginError::SessionNotFound
            }
        ),
        "expected SessionNotFound on empty signature, got: {err:?}"
    );

    user.session = "   ".to_string();
    user.session_signature = "valid_signature".to_string();
    let err = fetch_tradingview_token(&user).await.unwrap_err();
    assert!(
        matches!(
            err,
            Error::Login {
                source: LoginError::SessionNotFound
            }
        ),
        "expected SessionNotFound on whitespace session, got: {err:?}"
    );
}

#[tokio::test]
async fn test_login_with_captcha_rejects_empty_credentials_offline() {
    let mut user = UserCookies::new();

    let err = user
        .login_with_captcha("", "", None, "dummy_key")
        .await
        .unwrap_err();
    assert!(
        matches!(
            err,
            Error::Login {
                source: LoginError::EmptyCredentials
            }
        ),
        "expected EmptyCredentials on both empty, got: {err:?}"
    );

    let err = user
        .login_with_captcha("alice", "", None, "dummy_key")
        .await
        .unwrap_err();
    assert!(
        matches!(
            err,
            Error::Login {
                source: LoginError::EmptyCredentials
            }
        ),
        "expected EmptyCredentials on empty password, got: {err:?}"
    );
}

#[tokio::test]
async fn test_login_with_captcha_rejects_blank_api_key_offline() {
    let mut user = UserCookies::new();

    let err = user
        .login_with_captcha("alice", "secret", None, "")
        .await
        .unwrap_err();
    assert!(
        matches!(err, Error::Request(_)),
        "expected Error::Request on empty api key, got: {err:?}"
    );

    let err = user
        .login_with_captcha("alice", "secret", None, "   ")
        .await
        .unwrap_err();
    assert!(
        matches!(err, Error::Request(_)),
        "expected Error::Request on whitespace api key, got: {err:?}"
    );
}
