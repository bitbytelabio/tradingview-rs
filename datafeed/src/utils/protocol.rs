use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{de, Value};
use std::error::Error;
use tracing::debug;
use tracing_subscriber::fmt::format;
use tungstenite::protocol::Message;
// Define the structure of a Packet
#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct Packet {
    pub p: Value,
    pub m: Value,
}
// Define two regular expressions for cleaning and splitting the message
lazy_static::lazy_static! {
    static ref CLEANER_RGX: Regex = Regex::new(r"~h~").unwrap();
    static ref SPLITTER_RGX: Regex = Regex::new(r"~m~[0-9]{1,}~m~").unwrap();
}
// Parse the packet from the message
pub fn parse_packet(message: &str) -> Result<Vec<Packet>, Box<dyn Error>> {
    if message.is_empty() {
        return Err("Empty message".into());
    }
    let cleaned_message = CLEANER_RGX.replace_all(message, "");
    let packets: Vec<Packet> = SPLITTER_RGX
        .split(&cleaned_message)
        .filter(|x| {
            !x.is_empty() && !(x.contains("studies_metadata_hash") && x.contains("javastudies"))
        })
        .map(|x| {
            let packet: Packet = match serde_json::from_str(&x) {
                Ok(p) => p,
                Err(e) => {
                    tracing::error!("Error parsing packet: {}", e);
                    Packet {
                        p: Value::Null,
                        m: Value::Null,
                    }
                }
            };
            packet
        })
        .filter(|x| x.p != Value::Null || x.m != Value::Null)
        .collect();
    Ok(packets)
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
                debug!("Error formatting packet: {}", e);
                String::from("")
            }
        },
        Err(e) => {
            debug!("Error formatting packet: {}", e);
            String::from("")
        }
    };
    let formatted_msg = format!("~m~{}~m~{}", msg.len(), msg.as_str());
    debug!("Formatted packet: {}", formatted_msg);
    Message::Text(formatted_msg)
}
