use thiserror::Error;

#[derive(Debug, Error)]
pub enum LoginError {
    #[error("Invalid username or password")]
    LoginFailed,
    #[error("TOTP Secret is empty or invalid")]
    InvalidTOTP,
    #[error("MFA login failed")]
    MFALoginFailed,
    #[error("can not parse user data")]
    ParseUserDataError,
    #[error("can not parse auth token")]
    ParseAuthTokenError,
    #[error("Wrong or expired sessionid/signature")]
    SessionExpired,
}
