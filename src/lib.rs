pub mod chart;
pub mod client;
pub mod error;
pub mod handler;
pub mod models;
pub mod prelude;
pub mod quote;
pub mod socket;

#[cfg(feature = "user")]
pub mod user;

mod utils;
static UA: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/138.0.0.0 Safari/537.36";

pub use crate::client::misc::*;

pub use chart::history;

pub mod websocket {
    pub use crate::client::websocket::*;
}

pub use crate::models::*;

pub type Result<T> = std::result::Result<T, Error>;

pub use error::Error;

// Re-exporting some commonly used types
pub use async_trait::async_trait;
pub use iso_currency::{Country, Currency, CurrencySymbol};
