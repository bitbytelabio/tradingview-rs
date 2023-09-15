pub mod chart;
pub mod client;
pub mod error;
pub mod models;
pub mod quote;
pub mod socket;

pub mod user;

mod prelude;
mod session;
mod utils;
static UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/118.0.0.0 Safari/537.36";

pub mod tools {
    pub use crate::chart::utils::{
        extract_ohlcv_data, par_extract_ohlcv_data, sort_ohlcv_tuples, update_ohlcv_data,
        update_ohlcv_data_point,
    };
    pub use crate::utils::{clean_em_tags, format_packet, parse_compressed, parse_packet};
}

pub mod api {}
