use reqwest::header::{HeaderMap, HeaderValue};

pub mod chart;
pub mod client;
pub mod error;
pub mod model;
mod prelude;
pub mod quote;
pub mod socket;
pub mod user;
pub mod utils;

static UA: &'static str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/115.0.0.0 Safari/537.36";

const MESSAGE_TYPE_KEY: &'static str = "m";
const MESSAGE_PAYLOAD_KEY: &'static str = "p";
const ERROR_EVENT: &'static str = "critical_error";

lazy_static::lazy_static! {
    static ref WEBSOCKET_HEADERS: HeaderMap<HeaderValue> = {
        let mut headers = HeaderMap::new();
        headers.insert("Origin", "https://www.tradingview.com/".parse().unwrap());
        headers.insert("User-Agent", UA.parse().unwrap());
        headers
    };
}

#[macro_export]
macro_rules! payload {
    ($($payload:expr),*) => {{
        let payload_vec = vec![$(serde_json::Value::from($payload)),*];
        payload_vec
    }};
}
