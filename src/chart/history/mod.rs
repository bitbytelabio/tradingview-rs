use crate::{Error, Result, error::LoginError};

pub mod batch;
pub mod single;

/// Resolve authentication token from parameter or environment
fn resolve_auth_token(auth_token: Option<&str>) -> Result<String> {
    match auth_token {
        Some(token) => Ok(token.to_string()),
        None => {
            tracing::warn!("No auth token provided, using environment variable");
            std::env::var("TV_AUTH_TOKEN").map_err(|_| {
                tracing::error!("TV_AUTH_TOKEN environment variable is not set");
                Error::LoginError(LoginError::InvalidSession)
            })
        }
    }
}
