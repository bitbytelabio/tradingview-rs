use rand::Rng;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, COOKIE, ORIGIN, REFERER};
use reqwest::{Client, Error, Response};
use std::error::Error as StdError;
use tracing::{debug, error, info};

pub mod protocol;

#[tracing::instrument]
pub async fn get_request(url: &str, cookies: Option<String>) -> Result<Response, Error> {
    info!("Sending request to: {}", url);
    let client = Client::builder()
        .use_rustls_tls()
        .default_headers({
            let mut headers = HeaderMap::new();
            headers.insert(
                ACCEPT,
                HeaderValue::from_static("application/json, text/plain, */*"),
            );
            headers.insert(
                ORIGIN,
                HeaderValue::from_static("https://www.tradingview.com"),
            );
            headers.insert(
                REFERER,
                HeaderValue::from_static("https://www.tradingview.com/"),
            );
            headers
        })
        .https_only(true)
        .user_agent(crate::UA)
        .gzip(true)
        .build()?;

    let mut request = client.get(url);
    request = match cookies {
        Some(cookies) => {
            let mut headers = HeaderMap::new();
            headers.insert(
                COOKIE,
                match HeaderValue::from_str(&cookies) {
                    Ok(header_value) => header_value,
                    Err(e) => {
                        error!("Error parsing cookies: {}", e);
                        HeaderValue::from_static("")
                    }
                },
            );
            request.headers(headers)
        }
        None => request,
    };
    debug!("Sending request: {:?}", request);
    let response = request.send().await?;
    Ok(response)
}

#[tracing::instrument]
pub fn gen_session_id(session_type: &str) -> String {
    let mut rng = rand::thread_rng();
    let characters: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let result: String = (0..12)
        .map(|_| {
            let random_index = rng.gen_range(0..characters.len());
            characters[random_index] as char
        })
        .collect();

    format!("{}_{}", session_type, result)
}
