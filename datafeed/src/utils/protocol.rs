use lazy_static::lazy_static;
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
lazy_static! {
    static ref CLEANER_RGX: Regex = Regex::new(r"~h~").unwrap();
    static ref SPLITTER_RGX: Regex = Regex::new(r"~m~[0-9]{1,}~m~").unwrap();
}
// Parse the packet from the message
pub fn parse_packet(message: &str) -> Result<Vec<Packet>, Box<dyn Error>> {
    if message.is_empty() {
        return Err("Empty message".into());
    }
    let packets: Vec<Packet> = SPLITTER_RGX
        .split(message)
        .filter(|x| !x.is_empty())
        .map(|x| {
            let cleaned = CLEANER_RGX.replace_all(x, "");
            let packet: Packet =
                serde_json::from_str(&cleaned).expect("Failed to parse packet from message");
            packet
        })
        .collect();
    Ok(packets)
}
// Format the packet into a message
pub fn format_packet(packet: &Packet) -> Result<String, Box<dyn Error>> {
    let msg = serde_json::to_string(&packet)?;
    Ok(format!("~m~{}~m~{}", msg.len(), msg))
}
