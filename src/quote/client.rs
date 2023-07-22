// use crate::{model::SocketMessage, user::User};
// use serde_json::to_value;
// use serde_json::Value;

// pub struct QuoteSocket {
//     pub user: User,
//     pub session: String,
// }

// impl QuoteSocket {
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
//                     to_value(crate::quote::ALL_QUOTE_FIELDS.clone()).unwrap(),
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
