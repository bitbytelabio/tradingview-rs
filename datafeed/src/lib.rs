pub mod auth;
pub mod misc_requests;

static UA: &'static str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/112.0.0.0 Safari/537.36 uacq";

mod client;
mod utils;

extern crate google_authenticator;
extern crate regex;

#[macro_use]
extern crate lazy_static;
