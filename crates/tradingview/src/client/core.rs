use std::sync::Arc;

pub trait DataClient {
    fn new(auth_token: Option<&str>) -> Arc<Self>;
}

/// Validates an HTTP response status code, returning `Error::RateLimited` on 429
/// and `Error::Request` on other non-2xx status codes.
pub fn validate_response_status(response: &wreq::Response) -> Result<(), crate::Error> {
    let status = response.status();
    if status == wreq::StatusCode::TOO_MANY_REQUESTS {
        return Err(crate::Error::RateLimited(ustr::Ustr::from(&format!(
            "HTTP 429 Too Many Requests: {}",
            response.uri()
        ))));
    }
    if !status.is_success() {
        return Err(crate::Error::Request(ustr::Ustr::from(&format!(
            "HTTP request failed with status {}: {}",
            status,
            response.uri()
        ))));
    }
    Ok(())
}
