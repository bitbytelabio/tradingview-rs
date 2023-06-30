use datafeed::client::SocketMessage;
use datafeed::utils::protocol::{format_packet, parse_packet};
use futures_util::SinkExt;
use serde_json::Value;
use tracing::debug;
use tungstenite::{client::IntoClientRequest, connect, stream::NoDelay, Message};

#[tokio::main]
async fn main() {
    // Initialize tracing subscriber
    tracing_subscriber::fmt::init();
    let server = datafeed::client::DataServer::Data;
    let mut client = datafeed::client::Socket::new(server, None);
    let session = datafeed::utils::gen_session_id("cs");
    let messages = vec![
        SocketMessage {
            m: "quote_create_session".to_string(),
            p: vec![session.clone().into()],
        },
        SocketMessage {
            m: "quote_set_fields".to_string(),
            p: vec![
                session.clone().into(),
                "lb".into(),
                "volume".into(),
                "bid".into(),
                "ask".into(),
            ],
        },
        SocketMessage {
            m: "quote_add_symbols".to_string(),
            p: vec![session.clone().into(), "BINANCE:BTCUSDT".into()],
        },
    ];
    messages.iter().for_each(|msg| {
        let _ = client.send(msg);
    });
    client.read_message();
    // let raw = r#"~m~559~m~{"m":"qsd","p":["qs_snapshoter_basic-symbol-quotes_2btz4hzMC5lc",{"n":"TVC:DE10Y","s":"ok","v":{"visible-plots-set":"ohlc","update_mode":"streaming","typespecs":["government","yield","benchmark"],"type":"bond","symbol-primaryname":"TVC:DE10Y","source-logoid":"provider/tvc","short_name":"DE10Y","provider_id":"refinitiv","pro_name":"TVC:DE10Y","pricescale":1000,"minmov":1,"logoid":"country/DE","listed_exchange":"TVC","fractional":false,"exchange":"TVC","description":"Germany 10 Year Government Bonds Yield","country_code":"DE","base_name":["TVC:DE10Y"]}}]}"#;
    // let packets = parse_packet(raw);
    // debug!("{:?}", packets);
    // let msg = format_packet(packets[0].clone());
    // debug!("{:?}", msg.to_string());
}
