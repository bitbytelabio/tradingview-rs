use datafeed::utils::protocol::{format_ws_packet, parse_ws_packet, SerializedPacket};

fn main() {
    let msg: SerializedPacket<String> = SerializedPacket {
        p: "chart_create_session".to_string(),
        m: vec!["asdffd".to_string()],
    };

    let msg = format_ws_packet(msg);
    println!("{}", msg);
}
