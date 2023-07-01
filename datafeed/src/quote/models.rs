pub struct QuoteSession {
    pub session: String,
}

lazy_static::lazy_static! {
    static ref QUOTE_FIELDS: Vec<&'static str> = vec!["base-currency-logoid", "ch", "chp", "currency-logoid",
    "currency_code", "current_session", "description",
    "exchange", "format", "fractional", "is_tradable",
    "language", "local_description", "logoid", "lp",
    "lp_time", "minmov", "minmove2", "original_name",
    "pricescale", "pro_name", "short_name", "type",
    "update_mode", "volume", "ask", "bid", "fundamentals",
    "high_price", "low_price", "open_price", "prev_close_price",
    "rch", "rchp", "rtc", "rtc_time", "status", "industry",
    "basic_eps_net_income", "beta_1_year", "market_cap_basic",
    "earnings_per_share_basic_ttm", "price_earnings_ttm",
    "sector", "dividends_yield", "timezone", "country_code",
    "provider_id"];
}
