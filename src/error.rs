use serde::{Deserialize, Serialize};
use thiserror::Error;
use ustr::Ustr;

#[derive(Debug, Clone, Error, Copy, Serialize, Deserialize)]
pub enum Error {
    #[error("Generic: {0}")]
    Generic(Ustr),

    #[error("Request failed: {0}")]
    Request(Ustr),

    #[error("JSON parsing failed: {0}")]
    JsonParse(Ustr),

    #[error("Type conversion failed: {0}")]
    TypeConversion(Ustr),

    #[error("Invalid header value: {0}")]
    HeaderValue(Ustr),

    #[error("Login failed: {source}")]
    Login {
        #[source]
        source: LoginError,
    },

    #[error("Regex error: {0}")]
    Regex(Ustr),

    #[error("WebSocket connection failed: {0}")]
    WebSocket(Ustr),

    #[error("No chart token found")]
    NoChartTokenFound,

    #[error("No scan data found")]
    NoScanDataFound,

    #[error("Symbols are not in the same exchange")]
    SymbolsNotInSameExchange,

    #[error("Exchange not specified")]
    ExchangeNotSpecified,

    #[error("Invalid exchange")]
    InvalidExchange,

    #[error("Symbols not specified")]
    SymbolsNotSpecified,

    #[error("No search data found")]
    NoSearchDataFound,

    #[error("Indicator not found or unsupported: {0}")]
    IndicatorDataNotFound(Ustr),

    #[error("Task join failed: {0}")]
    TokioJoin(Ustr),

    #[error("URL parsing failed: {0}")]
    UrlParse(Ustr),

    #[error("Base64 decode failed: {0}")]
    Base64Decode(Ustr),

    #[error("ZIP error: {0}")]
    Zip(Ustr),

    #[error("Date/time parsing failed: {0}")]
    ChronoParse(Ustr),

    #[error("Date/time out of range: {0}")]
    ChronoOutOfRange(Ustr),

    #[error("Timeout: {0}")]
    Timeout(Ustr),

    #[error("I/O error: {0}")]
    Io(Ustr),

    #[error("TradingView error: {source}")]
    TradingView {
        #[source]
        source: TradingViewError,
    },
}

// Implement From traits for common error types
impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Self {
        Error::Request(err.to_string().into())
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::JsonParse(err.to_string().into())
    }
}

impl From<std::num::ParseIntError> for Error {
    fn from(err: std::num::ParseIntError) -> Self {
        Error::TypeConversion(err.to_string().into())
    }
}

impl From<reqwest::header::InvalidHeaderValue> for Error {
    fn from(err: reqwest::header::InvalidHeaderValue) -> Self {
        Error::HeaderValue(err.to_string().into())
    }
}

impl From<LoginError> for Error {
    fn from(err: LoginError) -> Self {
        Error::Login { source: err }
    }
}

impl From<regex::Error> for Error {
    fn from(err: regex::Error) -> Self {
        Error::Regex(err.to_string().into())
    }
}

impl From<tokio_tungstenite::tungstenite::Error> for Error {
    fn from(err: tokio_tungstenite::tungstenite::Error) -> Self {
        Error::WebSocket(err.to_string().into())
    }
}

impl From<tokio::task::JoinError> for Error {
    fn from(err: tokio::task::JoinError) -> Self {
        Error::TokioJoin(err.to_string().into())
    }
}

impl From<url::ParseError> for Error {
    fn from(err: url::ParseError) -> Self {
        Error::UrlParse(err.to_string().into())
    }
}

impl From<base64::DecodeError> for Error {
    fn from(err: base64::DecodeError) -> Self {
        Error::Base64Decode(err.to_string().into())
    }
}

impl From<zip::result::ZipError> for Error {
    fn from(err: zip::result::ZipError) -> Self {
        Error::Zip(err.to_string().into())
    }
}

impl From<chrono::ParseError> for Error {
    fn from(err: chrono::ParseError) -> Self {
        Error::ChronoParse(err.to_string().into())
    }
}

impl From<chrono::OutOfRangeError> for Error {
    fn from(err: chrono::OutOfRangeError) -> Self {
        Error::ChronoOutOfRange(err.to_string().into())
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err.to_string().into())
    }
}

impl From<TradingViewError> for Error {
    fn from(err: TradingViewError) -> Self {
        Error::TradingView { source: err }
    }
}

#[derive(Debug, Clone, Error, PartialEq, Eq, Hash, Copy, Serialize, Deserialize)]
pub enum TradingViewError {
    #[error("Series error")]
    SeriesError,
    #[error("Symbol error")]
    SymbolError,
    #[error("Critical error")]
    CriticalError,
    #[error("Study error")]
    StudyError,
    #[error("Protocol error")]
    ProtocolError,
    #[error("Quote data status error: {0}")]
    QuoteDataStatusError(Ustr),
    #[error("Replay error")]
    ReplayError,
    #[error("Configuration error: missing exchange")]
    MissingExchange,
    #[error("Configuration error: missing symbol")]
    MissingSymbol,
    #[error("Invalid session ID or signature")]
    InvalidSessionId,
}

#[derive(Debug, Clone, Error, PartialEq, Eq, Hash, Copy, Serialize, Deserialize)]
pub enum LoginError {
    #[error("Username or password is empty")]
    EmptyCredentials,
    #[error("Username or password is invalid")]
    InvalidCredentials,
    #[error("OTP secret is empty")]
    OTPSecretNotFound,
    #[error("OTP secret is invalid")]
    InvalidOTPSecret,
    #[error("Wrong or expired session ID/signature")]
    InvalidSession,
    #[error("Session ID/signature is empty")]
    SessionNotFound,
    #[error("Cannot parse user ID")]
    ParseIDError,
    #[error("Cannot parse username")]
    ParseUsernameError,
    #[error("Cannot parse session hash")]
    ParseSessionHashError,
    #[error("Cannot parse private channel")]
    ParsePrivateChannelError,
    #[error("Cannot parse auth token")]
    ParseAuthTokenError,
    #[error("Missing auth token")]
    MissingAuthToken,
}
