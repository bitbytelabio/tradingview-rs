use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Generic {0}")]
    Generic(String),

    #[error("failed to send the api request")]
    RequestError(#[from] reqwest::Error),

    #[error("failed to parse the api response")]
    ParseError(#[from] serde_json::Error),

    #[error("failed to convert into int from {}", .0)]
    TypeConversionError(#[from] std::num::ParseIntError),

    #[error("invalid header value")]
    HeaderValueError(#[from] reqwest::header::InvalidHeaderValue),

    #[error("failed to login")]
    LoginError(#[from] LoginError),

    #[error("failed to capture regex data")]
    RegexError(#[from] regex::Error),

    #[error("something went wrong with websocket")]
    WebSocketError(#[from] tokio_tungstenite::tungstenite::Error),

    #[error("No chart token found")]
    NoChartTokenFound,

    #[error("No scan data found")]
    NoScanDataFound,

    #[error("Symbols may not in the same exchange")]
    SymbolsNotInSameExchange,

    #[error("Exchange not specified")]
    ExchangeNotSpecified,

    #[error("Exchange is invalid")]
    InvalidExchange,

    #[error("Symbols not specified")]
    SymbolsNotSpecified,

    #[error("No search data found")]
    NoSearchDataFound,

    #[error("Inexistent or unsupported indicator {}", .0)]
    IndicatorDataNotFound(String),

    #[error("Tokio task join error")]
    TokioJoinError(#[from] tokio::task::JoinError),
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
