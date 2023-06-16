pub mod auth;
mod config;
pub mod misc_requests;
mod utils;

pub static UA: &'static str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/112.0.0.0 Safari/537.36 uacq";

extern crate google_authenticator;
extern crate regex;

#[macro_use]
extern crate lazy_static;

use std::io::Read;
use zip::read::ZipFile;
