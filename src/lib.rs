pub mod callback;
pub mod chart;
pub mod client;
pub mod data_loader;
pub mod error;
pub mod models;
pub mod quote;
pub mod socket;

#[cfg(feature = "user")]
pub mod user;

mod utils;
static UA: &str =
    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/117.0.0.0 Safari/537.36";

pub mod api {
    pub use crate::client::misc::{
        get_builtin_indicators, get_chart_token, get_drawing, get_indicator_metadata,
        get_private_indicators, get_quote_token, list_symbols, search_indicator, search_symbol,
    };
}

pub type Result<T> = std::result::Result<T, error::Error>;

pub use error::Error;
