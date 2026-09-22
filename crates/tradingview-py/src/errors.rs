use pyo3::create_exception;
use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use tradingview::Error;
use tradingview::error::{LoginError, TradingViewError as TvError};

// Python exception hierarchy
create_exception!(tradingview, TradingViewError, PyException);
create_exception!(tradingview, AuthenticationError, TradingViewError);
create_exception!(tradingview, SymbolNotFoundError, TradingViewError);
create_exception!(tradingview, ConnectionError, TradingViewError);
create_exception!(tradingview, TimeoutError, TradingViewError);
create_exception!(tradingview, RateLimitError, TradingViewError);
create_exception!(tradingview, ProtocolError, TradingViewError);

/// Register all exception classes on the Python module
pub fn register_exceptions(py: Python<'_>, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("TradingViewError", py.get_type::<TradingViewError>())?;
    m.add("AuthenticationError", py.get_type::<AuthenticationError>())?;
    m.add("SymbolNotFoundError", py.get_type::<SymbolNotFoundError>())?;
    m.add("ConnectionError", py.get_type::<ConnectionError>())?;
    m.add("TimeoutError", py.get_type::<TimeoutError>())?;
    m.add("RateLimitError", py.get_type::<RateLimitError>())?;
    m.add("ProtocolError", py.get_type::<ProtocolError>())?;
    Ok(())
}

/// Orphan-rule-safe mapping from `tradingview::Error` to `PyErr`
pub fn to_py_err(err: Error) -> PyErr {
    match err {
        Error::RateLimited(msg) => RateLimitError::new_err(msg.to_string()),
        Error::Login { source } => match source {
            LoginError::EmptyCredentials | LoginError::InvalidCredentials => {
                AuthenticationError::new_err("Invalid credentials")
            }
            LoginError::OTPSecretNotFound | LoginError::InvalidOTPSecret => {
                AuthenticationError::new_err("Invalid or missing OTP secret")
            }
            LoginError::InvalidSession | LoginError::SessionNotFound => {
                AuthenticationError::new_err("Session expired or invalid")
            }
            LoginError::ParseAuthTokenError | LoginError::MissingAuthToken => {
                AuthenticationError::new_err("Auth token error")
            }
            LoginError::ParseIDError
            | LoginError::ParseUsernameError
            | LoginError::ParseSessionHashError
            | LoginError::ParsePrivateChannelError => {
                ProtocolError::new_err("Login response parse failure")
            }
        },
        Error::NoChartTokenFound => AuthenticationError::new_err("No chart token found"),
        Error::TradingView { source } => match source {
            TvError::InvalidSessionId => {
                AuthenticationError::new_err("Invalid session ID or signature")
            }
            TvError::SymbolError => SymbolNotFoundError::new_err("Symbol resolution failed"),
            TvError::MissingSymbol => SymbolNotFoundError::new_err("Missing symbol configuration"),
            TvError::MissingExchange => {
                SymbolNotFoundError::new_err("Missing exchange configuration")
            }
            TvError::QuoteDataStatusError(s) => {
                let lower = s.as_str().to_lowercase();
                if lower.contains("limit") || lower.contains("throttled") || lower.contains("rate")
                {
                    RateLimitError::new_err(s.to_string())
                } else {
                    ProtocolError::new_err(s.to_string())
                }
            }
            TvError::ProtocolError => ProtocolError::new_err("Protocol error"),
            TvError::SeriesError
            | TvError::CriticalError
            | TvError::StudyError
            | TvError::ReplayError => {
                TradingViewError::new_err(format!("TradingView error: {source}"))
            }
        },
        Error::SymbolsNotInSameExchange => {
            SymbolNotFoundError::new_err("Symbols are not in the same exchange")
        }
        Error::ExchangeNotSpecified => SymbolNotFoundError::new_err("Exchange not specified"),
        Error::InvalidExchange => SymbolNotFoundError::new_err("Invalid exchange"),
        Error::SymbolsNotSpecified => SymbolNotFoundError::new_err("Symbols not specified"),
        Error::NoSearchDataFound => SymbolNotFoundError::new_err("No search data found"),
        Error::IndicatorDataNotFound(s) => {
            SymbolNotFoundError::new_err(format!("Indicator data not found: {s}"))
        }
        Error::NoScanDataFound => SymbolNotFoundError::new_err("No scan data found"),
        Error::WebSocket(s) => ConnectionError::new_err(format!("WebSocket error: {s}")),
        Error::Request(s) => ConnectionError::new_err(format!("HTTP request failed: {s}")),
        Error::Io(s) => ConnectionError::new_err(format!("I/O error: {s}")),
        Error::Timeout(s) => TimeoutError::new_err(format!("Operation timed out: {s}")),
        Error::JsonParse(s) => ProtocolError::new_err(format!("JSON parse error: {s}")),
        Error::TypeConversion(s) => ProtocolError::new_err(format!("Type conversion error: {s}")),
        Error::UrlParse(s) => ProtocolError::new_err(format!("URL parse error: {s}")),
        Error::Regex(s) => ProtocolError::new_err(format!("Regex error: {s}")),
        Error::Internal(s) => TradingViewError::new_err(format!("Internal error: {s}")),
        Error::TokioJoin(s) => TradingViewError::new_err(format!("Task join error: {s}")),
        Error::ChronoParse(s) | Error::ChronoOutOfRange(s) => {
            TradingViewError::new_err(format!("Date/time parse error: {s}"))
        }
        Error::HeaderValue(s) => TradingViewError::new_err(format!("Invalid header: {s}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ustr::Ustr;

    #[test]
    fn test_quote_data_status_rate_limit_match() {
        Python::initialize();
        Python::attach(|py| {
            let err = Error::TradingView {
                source: TvError::QuoteDataStatusError(Ustr::from("quote rate limit exceeded")),
            };
            let py_err = to_py_err(err);
            assert!(py_err.is_instance_of::<RateLimitError>(py));

            let err2 = Error::TradingView {
                source: TvError::QuoteDataStatusError(Ustr::from("request throttled by server")),
            };
            let py_err2 = to_py_err(err2);
            assert!(py_err2.is_instance_of::<RateLimitError>(py));
        });
    }

    #[test]
    fn test_quote_data_status_fallback_to_protocol_error() {
        Python::initialize();
        Python::attach(|py| {
            let err = Error::TradingView {
                source: TvError::QuoteDataStatusError(Ustr::from("unexpected status code")),
            };
            let py_err = to_py_err(err);
            assert!(py_err.is_instance_of::<ProtocolError>(py));
        });
    }

    #[test]
    fn test_rate_limited_variant() {
        Python::initialize();
        Python::attach(|py| {
            let err = Error::RateLimited(Ustr::from("429 Too Many Requests"));
            let py_err = to_py_err(err);
            assert!(py_err.is_instance_of::<RateLimitError>(py));
        });
    }

    #[test]
    fn test_auth_error_mapping() {
        Python::initialize();
        Python::attach(|py| {
            let err = Error::Login {
                source: LoginError::InvalidCredentials,
            };
            let py_err = to_py_err(err);
            assert!(py_err.is_instance_of::<AuthenticationError>(py));

            let err_token = Error::NoChartTokenFound;
            let py_err_token = to_py_err(err_token);
            assert!(py_err_token.is_instance_of::<AuthenticationError>(py));
        });
    }

    #[test]
    fn test_connection_and_timeout_error_mapping() {
        Python::initialize();
        Python::attach(|py| {
            let err_ws = Error::WebSocket(Ustr::from("connection reset"));
            let py_err_ws = to_py_err(err_ws);
            assert!(py_err_ws.is_instance_of::<ConnectionError>(py));

            let err_timeout = Error::Timeout(Ustr::from("5000ms elapsed"));
            let py_err_timeout = to_py_err(err_timeout);
            assert!(py_err_timeout.is_instance_of::<TimeoutError>(py));
        });
    }
}
