use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Generic {0}")]
    Generic(String),

    #[error("failed to send the api request")]
    RequestError(#[from] reqwest::Error),

    #[error("failed to parse the api response")]
    ParseError(#[from] serde_json::Error),

    #[error("failed to convert into int form {}", .0)]
    TypeConversionError(#[from] std::num::ParseIntError),

    #[error("invalid header value")]
    HeaderValueError(#[from] reqwest::header::InvalidHeaderValue),

    #[error("failed to login")]
    LoginError(#[from] LoginError),

    #[error("failed to capture regex data")]
    RegexError(#[from] regex::Error),

    #[error("something went wrong with websocket")]
    WebSocketError(#[from] tokio_tungstenite::tungstenite::Error),
}

#[derive(Debug, Error)]
pub enum LoginError {
    #[error("username or password is empty")]
    EmptyCredentials,

    #[error("username or password is invalid")]
    InvalidCredentials,

    #[error("OTP Secret is empty")]
    OTPSecretNotFound,

    #[error("OTP Secret is invalid")]
    InvalidOTPSecret,

    #[error("Wrong or expired sessionid/signature")]
    InvalidSession,

    #[error("Sessionid/signature is empty")]
    SessionNotFound,

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

    #[error("unable to parse message")]
    ParseMessageError,

    #[error("unable to send ping message")]
    PingError,
}
