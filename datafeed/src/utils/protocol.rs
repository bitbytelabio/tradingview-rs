use regex::Regex;
use serde::Serialize;
use serde_json::Value;
use tracing::{debug, warn};
use tungstenite::protocol::Message;
// Define two regular expressions for cleaning and splitting the message
lazy_static::lazy_static! {
    static ref CLEANER_RGX: Regex = Regex::new(r"~h~").unwrap();
    static ref SPLITTER_RGX: Regex = Regex::new(r"~m~[0-9]{1,}~m~").unwrap();
}
// Parse the packet from the message
pub fn parse_packet(message: &str) -> Vec<Value> {
    if message.is_empty() {
        vec![Value::Null];
    }
    let cleaned_message = CLEANER_RGX.replace_all(message, "");
    let packets: Vec<Value> = SPLITTER_RGX
        .split(&cleaned_message)
        .filter(|x| !x.is_empty())
        .map(|x| {
            let packet: Value = match serde_json::from_str(x) {
                Ok(packet) => packet,
                Err(e) => {
                    debug!("Error parsing packet: {}", e);
                    Value::Null
                }
            };
            packet
        })
        .filter(|x| x != &Value::Null)
        .collect();
    packets
}

// Format the packet into a message
pub fn format_packet<T>(packet: T) -> Message
where
    T: Serialize,
{
    let msg = match serde_json::to_value(&packet) {
        Ok(msg) => match serde_json::to_string(&msg) {
            Ok(msg) => msg,
            Err(e) => {
                warn!("Error formatting packet: {}", e);
                String::from("")
            }
        },
        Err(e) => {
            warn!("Error formatting packet: {}", e);
            String::from("")
        }
    };
    let formatted_msg = format!("~m~{}~m~{}", msg.len(), msg.as_str());
    debug!("Formatted packet: {}", formatted_msg);
    Message::Text(formatted_msg)
}
