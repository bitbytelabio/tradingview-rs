pub mod chart;
pub mod client;
pub mod error;
pub mod models;
pub mod prelude;
pub mod quote;

#[cfg(feature = "user")]
pub mod user;

mod utils;
static UA: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/138.0.0.0 Safari/537.36";

pub use crate::client::misc::*;

pub mod websocket {
    pub use crate::live::websocket::*;
}

pub use crate::live::models::*;
pub use crate::models::*;

pub type Result<T> = std::result::Result<T, Error>;

pub use error::Error;

// Re-exporting some commonly used types
pub use iso_currency::{Country, Currency, CurrencySymbol};

pub mod historical;
pub mod live;
