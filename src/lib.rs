extern crate google_authenticator;
extern crate regex;

mod client;

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

static UA: &'static str = "Mozilla/5.0 (Windows NT 10.0; WOW64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/113.0.5666.197 Safari/537.36";
