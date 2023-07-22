extern crate google_authenticator;
extern crate regex;

pub mod api;
pub mod chart;
pub mod client;
pub mod config;
pub mod errors;
pub mod model;
pub mod quote;
pub mod user;
pub mod utils;
pub mod socket;

static UA: &'static str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/115.0.0.0 Safari/537.36";
