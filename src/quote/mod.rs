//! Real-time quote data types and field definitions.
//!
//! TradingView delivers streaming quote updates via WebSocket. This module
//! defines the data model for those updates and the complete set of available
//! quote fields.
//!
//! [`ALL_QUOTE_FIELDS`] is a compile-time constant listing every field name
//! TradingView may include in a quote packet.

pub mod models;
pub mod utils;

/// All TradingView quote field names as a compile-time constant slice.
/// Zero runtime allocation — the data lives in the binary's `.rodata` section.
pub const ALL_QUOTE_FIELDS: &[&str] = &[
    "lp",
    "lp_time",
    "ask",
    "bid",
    "bid_size",
    "ask_size",
    "ch",
    "chp",
    "volume",
    "high_price",
    "low_price",
    "open_price",
    "prev_close_price",
    "currency_id",
    "current_session",
    "description",
    "exchange",
    "format",
    "fractional",
    "is_tradable",
    "language",
    "local_description",
    "logoid",
    "minmov",
    "minmove2",
    "original_name",
    "pricescale",
    "pro_name",
    "short_name",
    "type",
    "update_mode",
    "fundamentals",
    "rch",
    "rchp",
    "rtc",
    "rtc_time",
    "status",
    "industry",
    "basic_eps_net_income",
    "beta_1_year",
    "market_cap_basic",
    "earnings_per_share_basic_ttm",
    "price_earnings_ttm",
    "sector",
    "dividends_yield",
    "timezone",
    "country",
    "provider_id",
];
