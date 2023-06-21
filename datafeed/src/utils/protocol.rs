use lazy_static::lazy_static;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Number, Value};
use std::{collections::HashMap, error::Error};
use tracing::{debug, error, info, warn};

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq)]
pub struct Packet {
    pub p: Value,
    pub m: Value,
}

lazy_static! {
    static ref CLEANER_RGX: Regex = Regex::new(r"~h~").unwrap();
    static ref SPLITTER_RGX: Regex = Regex::new(r"~m~[0-9]{1,}~m~").unwrap();
}

#[tracing::instrument(skip(message))]
pub fn parse_packet(message: &str) -> Result<Vec<Packet>, Box<dyn Error>> {
    let packets: Vec<Packet> = SPLITTER_RGX
        .split(message)
        .filter(|x| !x.is_empty())
        .map(|x| {
            let cleaned = CLEANER_RGX.replace_all(x, "");
            let packet: Packet = serde_json::from_str(&cleaned).unwrap();
            packet
        })
        .collect();

    Ok(packets)
}

pub fn format_packet(packet: Packet) -> Result<String, Box<dyn Error>> {
    let msg = serde_json::to_string(&packet)?;
    Ok(format!("~m~{}~m~{}", msg.len(), msg))
}
