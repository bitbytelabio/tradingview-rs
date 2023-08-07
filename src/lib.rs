use reqwest::header::{HeaderMap, HeaderValue};

extern crate google_authenticator;
extern crate regex;

pub mod chart;
pub mod client;
pub mod error;
pub mod model;
mod prelude;
pub mod quote;
pub mod socket;
pub mod user;
pub mod utils;

static UA: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/115.0.0.0 Safari/537.36";

lazy_static::lazy_static! {
    static ref WEBSOCKET_HEADERS: HeaderMap<HeaderValue> = {
        let mut headers = HeaderMap::new();
        headers.insert("Origin", "https://www.tradingview.com/".parse().unwrap());
        headers.insert("User-Agent", UA.parse().unwrap());
        headers
    };
}
