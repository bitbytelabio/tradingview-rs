use serde::{Deserialize, Deserializer};

pub fn deserialize_string_default<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let s: String = Deserialize::deserialize(deserializer).unwrap_or_default();
    Ok(s)
}

pub fn deserialize_vec_default<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    let s: Vec<T> = Deserialize::deserialize(deserializer).unwrap_or_default();
    Ok(s)
}

pub fn deserialize_option_default<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    let s: Option<T> = Deserialize::deserialize(deserializer).unwrap_or_default();
    Ok(s)
}
