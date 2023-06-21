use regex::Regex;
use serde::{Deserialize, Serialize};
use std::error::Error;
use tracing::error;

pub type DeserializedPacket = (Option<String>, Option<(String, serde_json::Value)>);

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct SerializedPacket<T: Serialize> {
    pub p: String,
    pub m: Vec<T>,
}

lazy_static! {
    static ref CLEANER_RGX: Regex = Regex::new(r"~h~").unwrap();
    static ref SPLITTER_RGX: Regex = Regex::new(r"~m~[0-9]{1,}~m~").unwrap();
}

pub fn parse_ws_packet(data: &str) -> Result<Vec<DeserializedPacket>, Box<dyn Error>> {
    let packets: Vec<DeserializedPacket> = data
        .split(SPLITTER_RGX.as_str())
        .filter_map(|p| {
            if p.is_empty() {
                return None;
            }
            match serde_json::from_str::<serde_json::Value>(p) {
                Ok(json) => Some((
                    json["m"].as_str().map(|s| s.to_owned()),
                    json["p"].as_array().and_then(|arr| {
                        let session = arr.get(0)?.as_str()?.to_owned();
                        let data = arr.get(1)?.clone();
                        Some((session, data))
                    }),
                )),
                Err(json_error) => {
                    error!("Cant parse: {}", p);
                    error!("Error: {}", json_error);
                    None
                }
            }
        })
        .collect();

    Ok(packets)
}

pub fn format_ws_packet<T>(packet: SerializedPacket<T>) -> String
where
    T: Serialize,
{
    let msg = serde_json::to_string(&packet).unwrap();
    format!("~m~{}~m~{}", msg.len(), msg)
}
