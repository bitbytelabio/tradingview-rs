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

pub mod tools {
    pub use crate::chart::utils::{
        extract_ohlcv_data,
        par_extract_ohlcv_data,
        sort_ohlcv_tuples,
        update_ohlcv_data,
        update_ohlcv_data_point,
    };
}

pub mod api {
    pub use crate::client::mics::{
        search_symbol,
        get_chart_token,
        get_drawing,
        get_builtin_indicators,
        get_private_indicators,
        get_indicator_metadata,
        list_symbols,
        search_indicator,
        get_quote_token,
    };
}

pub type Result<T> = std::result::Result<T, error::Error>;

pub use error::Error;
