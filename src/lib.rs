extern crate google_authenticator;
extern crate regex;

pub mod user;

pub mod api;
pub mod auth;
pub mod chart;
pub mod config;
pub mod errors;
pub mod misc_requests;
pub mod model;
pub mod quote;
pub mod utils;
pub mod websocket;

static UA: &'static str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/115.0.0.0 Safari/537.36";
