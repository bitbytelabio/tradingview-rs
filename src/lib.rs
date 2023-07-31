extern crate google_authenticator;
extern crate regex;

pub mod chart;
pub mod client;
pub mod config;
pub mod error;
pub mod model;
pub mod prelude;
pub mod quote;
pub mod socket;
pub mod symbol;
pub mod user;
pub mod utils;

static UA: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/115.0.0.0 Safari/537.36";
