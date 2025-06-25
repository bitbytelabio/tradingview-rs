use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Generic {0}")]
    Generic(String),
    #[error("failed to send the api request")]
    RequestError(#[from] reqwest::Error),
    #[error("failed to parse the api response")]
    JsonParseError(#[from] serde_json::Error),
    #[error("failed to convert into int from {}", .0)]
    TypeConversionError(#[from] std::num::ParseIntError),
    #[error("invalid header value")]
    HeaderValueError(#[from] reqwest::header::InvalidHeaderValue),
    #[error("failed to login")]
    LoginError(#[from] LoginError),
    #[error("failed to capture regex data")]
    RegexError(#[from] regex::Error),
    #[error("can not establish websocket connection")]
    WebSocketError(#[from] Box<tokio_tungstenite::tungstenite::Error>),
    #[error("no chart token found")]
    NoChartTokenFound,
    #[error("No scan data found")]
    NoScanDataFound,
    #[error("symbols may not in the same exchange")]
    SymbolsNotInSameExchange,
    #[error("exchange not specified")]
    ExchangeNotSpecified,
    #[error("exchange is invalid")]
    InvalidExchange,
    #[error("symbols not specified")]
    SymbolsNotSpecified,
    #[error("no search data found")]
    NoSearchDataFound,
    #[error("inexistent or unsupported indicator {}", .0)]
    IndicatorDataNotFound(String),
    #[error("tokio task join error")]
    TokioJoinError(#[from] tokio::task::JoinError),
    #[error("url parse error")]
    UrlParseError(#[from] url::ParseError),
    #[error("base64 decode error")]
    Base64DecodeError(#[from] base64::DecodeError),
    #[error("zip error")]
    ZipError(#[from] Box<zip::result::ZipError>),

    #[error("chrono parse error")]
    ChronoParseError(#[from] chrono::ParseError),
    #[error("chrono out of range error")]
    ChronoOutOfRangeError(#[from] chrono::OutOfRangeError),

    #[error("Timeout: {0}")]
    TimeoutError(String),

    #[error("I/O error")]
    IoError(#[from] std::io::Error),

    #[error("TradingView error")]
    TradingViewError(#[from] TradingViewError),
}

#[derive(Debug, Clone, Error, PartialEq, Eq, Hash)]
pub enum TradingViewError {
    #[error("series_error")]
    SeriesError,
    #[error("symbol_error")]
    SymbolError,
    #[error("critical_error")]
    CriticalError,
    #[error("study_error")]
    StudyError,
    #[error("protocol_error")]
    ProtocolError,
    #[error("error")]
    QuoteDataStatusError,
    #[error("replay_error")]
    ReplayError,
}

#[derive(Debug, Clone, Error)]
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

impl From<tokio_tungstenite::tungstenite::Error> for Error {
    fn from(err: tokio_tungstenite::tungstenite::Error) -> Self {
        Error::WebSocketError(Box::new(err))
    }
}

impl From<zip::result::ZipError> for Error {
    fn from(err: zip::result::ZipError) -> Self {
        Error::ZipError(Box::new(err))
    }
}
