use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::error::Error;
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
pub fn format_packet(packet: &Packet) -> Result<String, Box<dyn Error>> {
    let msg = serde_json::to_string(&packet)?;
    Ok(format!("~m~{}~m~{}", msg.len(), msg))
}
