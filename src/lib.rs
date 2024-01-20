pub mod callback;
pub mod chart;
pub mod client;
pub mod error;
pub mod models;
pub mod quote;
pub mod socket;

#[cfg(feature = "user")]
pub mod user;

mod utils;
static UA: &str =
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/117.0.0.0 Safari/537.36";

pub use crate::client::misc::{
    advanced_search_symbol, get_builtin_indicators, get_chart_token, get_drawing,
    get_indicator_metadata, get_private_indicators, get_quote_token, list_symbols,
    search_indicator,
};

pub mod websocket {
    pub use crate::client::websocket::*;
}

pub use crate::models::*;

pub type Result<T> = std::result::Result<T, Error>;

pub use error::Error;
