use thiserror::Error;

#[derive(Debug, Error)]
pub enum LoginError {
    #[error("username or password is empty")]
    EmptyCredentials,

    #[error("username or password is invalid")]
    LoginFailed,

    #[error("TOTP Secret is empty")]
    TOTPEmpty,

    #[error("unable to authenticate user with mfa")]
    AuthenticationMFAError,

    #[error("can not parse user id")]
    ParseIDError,

    #[error("can not parse username")]
    ParseUsernameError,

    #[error("can not parse session hash")]
    ParseSessionHashError,

    #[error("can not parse private channel")]
    ParsePrivateChannelError,

    #[error("can not parse auth token")]
    ParseAuthTokenError,

    #[error("Wrong or expired sessionid/signature")]
    SessionExpired,

    #[error("Sessionid/signature not found")]
    SessionNotFound,
}

#[derive(Debug, Error)]
pub enum ClientError {
    #[error("Empty message")]
    EmptyMessage,

    #[error("No token found")]
    NoTokenFound,

    #[error("No chart token found")]
    NoChartTokenFound,
}

#[derive(Debug, Error)]
pub enum SocketError {
    #[error("unable to establish websocket connection")]
    SocketConnectionError,
}
