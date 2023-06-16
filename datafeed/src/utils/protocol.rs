use regex::Regex;
use std::error::Error;
use tracing::error;

pub type TWPacket = (Option<String>, Option<(String, serde_json::Value)>);

lazy_static! {
    static ref CLEANER_RGX: Regex = Regex::new(r"~h~").unwrap();
    static ref SPLITTER_RGX: Regex = Regex::new(r"~m~[0-9]{1,}~m~").unwrap();
}

pub fn parse_ws_packet(data: &str) -> Result<Vec<TWPacket>, Box<dyn Error>> {
    let packets: Vec<TWPacket> = data
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

pub fn format_ws_packet(packet: &str) -> String {
    format!("~m~{}~m~{}", packet.len(), packet)
}
