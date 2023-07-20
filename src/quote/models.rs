// pub struct QuoteSession {
//     pub session: String,
// }

// lazy_static::lazy_static! {
//     static ref QUOTE_FIELDS: Vec<&'static str> = vec![
//         "base-currency-logoid", "ch", "chp", "currency-logoid",
//         "currency_code", "current_session", "description",
//         "exchange", "format", "fractional", "is_tradable",
//         "language", "local_description", "logoid", "lp",
//         "lp_time", "minmov", "minmove2", "original_name",
//         "pricescale", "pro_name", "short_name", "type",
//         "update_mode", "volume", "ask", "bid", "fundamentals",
//         "high_price", "low_price", "open_price", "prev_close_price",
//         "rch", "rchp", "rtc", "rtc_time", "status", "industry",
//         "basic_eps_net_income", "beta_1_year", "market_cap_basic",
//         "earnings_per_share_basic_ttm", "price_earnings_ttm",
//         "sector", "dividends_yield", "timezone", "country_code",
//         "provider_id"
//     ];
// }

// use serde_json::to_value;
// use serde_json::Value;

// use crate::client::SocketMessage;

// impl QuoteSession {
//     pub fn new(session: String) -> Self {
//         Self { session }
//     }
//     pub fn load() {
//         let server = crate::client::DataServer::Data;
//         let mut client = crate::client::Socket::new(server, None);
//         let session = crate::utils::gen_session_id("cs");
//         let messages = vec![
//             SocketMessage {
//                 m: "quote_create_session".to_string(),
//                 p: vec![session.clone().into()],
//             },
//             SocketMessage {
//                 m: "quote_set_fields".to_string(),
//                 p: vec![
//                     session.clone().into(),
//                     "base-currency-logoid".into(),
//                     "ch".into(),
//                     "chp".into(),
//                     "currency-logoid".into(),
//                     "currency_code".into(),
//                     "current_session".into(),
//                     "description".into(),
//                     "exchange".into(),
//                     "format".into(),
//                     "fractional".into(),
//                     "is_tradable".into(),
//                     "language".into(),
//                     "local_description".into(),
//                     "logoid".into(),
//                     "lp".into(),
//                     "lp_time".into(),
//                     "minmov".into(),
//                     "minmove2".into(),
//                     "original_name".into(),
//                     "pricescale".into(),
//                     "pro_name".into(),
//                     "short_name".into(),
//                     "type".into(),
//                     "update_mode".into(),
//                     "volume".into(),
//                     "ask".into(),
//                     "bid".into(),
//                     "fundamentals".into(),
//                     "high_price".into(),
//                     "low_price".into(),
//                     "open_price".into(),
//                     "prev_close_price".into(),
//                     "rch".into(),
//                     "rchp".into(),
//                     "rtc".into(),
//                     "rtc_time".into(),
//                     "status".into(),
//                     "industry".into(),
//                     "basic_eps_net_income".into(),
//                     "beta_1_year".into(),
//                     "market_cap_basic".into(),
//                     "earnings_per_share_basic_ttm".into(),
//                     "price_earnings_ttm".into(),
//                     "sector".into(),
//                     "dividends_yield".into(),
//                     "timezone".into(),
//                     "country_code".into(),
//                     "provider_id".into(),
//                 ],
//             },
//             SocketMessage {
//                 m: "quote_add_symbols".to_string(),
//                 p: vec![session.clone().into(), "BINANCE:BTCUSDT".into()],
//             },
//         ];
//         messages.iter().for_each(|msg| {
//             let _ = client.send(msg);
//         });
//         client.read_message();
//     }
// }
