use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, PartialEq, Deserialize, Serialize)]
pub struct QuoteData {
    #[serde(rename(deserialize = "n"))]
    pub name: String,
    #[serde(rename(deserialize = "s"))]
    pub status: String,
    #[serde(rename(deserialize = "v"))]
    pub value: QuoteValue,
}

#[derive(Clone, PartialEq, Deserialize, Serialize)]
#[cfg_attr(feature = "protobuf", derive(prost::Message))]
#[cfg_attr(not(feature = "protobuf"), derive(Debug, Default))]
pub struct QuoteValue {
    #[cfg_attr(feature = "protobuf", prost(double, optional, tag = "1"))]
    #[serde(default)]
    pub ask: Option<f64>,
    #[cfg_attr(feature = "protobuf", prost(double, optional, tag = "2"))]
    #[serde(default)]
    pub ask_size: Option<f64>,
    #[cfg_attr(feature = "protobuf", prost(double, optional, tag = "3"))]
    #[serde(default)]
    pub bid: Option<f64>,
    #[cfg_attr(feature = "protobuf", prost(double, optional, tag = "4"))]
    #[serde(default)]
    pub bid_size: Option<f64>,
    #[cfg_attr(feature = "protobuf", prost(double, optional, tag = "5"))]
    #[serde(default, rename(deserialize = "ch"))]
    pub change: Option<f64>,
    #[cfg_attr(feature = "protobuf", prost(double, optional, tag = "6"))]
    #[serde(default, rename(deserialize = "chp"))]
    pub change_percent: Option<f64>,
    #[cfg_attr(feature = "protobuf", prost(double, optional, tag = "7"))]
    #[serde(default, rename(deserialize = "open_price"))]
    pub open: Option<f64>,
    #[cfg_attr(feature = "protobuf", prost(double, optional, tag = "8"))]
    #[serde(default, rename(deserialize = "high_price"))]
    pub high: Option<f64>,
    #[cfg_attr(feature = "protobuf", prost(double, optional, tag = "9"))]
    #[serde(default, rename(deserialize = "low_price"))]
    pub low: Option<f64>,
    #[cfg_attr(feature = "protobuf", prost(double, optional, tag = "10"))]
    #[serde(default, rename(deserialize = "prev_close_price"))]
    pub prev_close: Option<f64>,
    #[cfg_attr(feature = "protobuf", prost(double, optional, tag = "11"))]
    #[serde(default, rename(deserialize = "lp"))]
    pub price: Option<f64>,
    #[cfg_attr(feature = "protobuf", prost(double, optional, tag = "12"))]
    #[serde(default, rename(deserialize = "lp_time"))]
    pub timestamp: Option<f64>,
    #[cfg_attr(feature = "protobuf", prost(double, optional, tag = "13"))]
    #[serde(default)]
    pub volume: Option<f64>,
    #[cfg_attr(feature = "protobuf", prost(string, optional, tag = "14"))]
    #[serde(default, rename(deserialize = "currency_id"))]
    pub currency: Option<String>,
    #[cfg_attr(feature = "protobuf", prost(string, optional, tag = "15"))]
    #[serde(default, rename(deserialize = "short_name"))]
    pub symbol: Option<String>,
    #[cfg_attr(feature = "protobuf", prost(string, optional, tag = "16"))]
    #[serde(default, rename(deserialize = "exchange"))]
    pub exchange: Option<String>,
    #[cfg_attr(feature = "protobuf", prost(string, optional, tag = "17"))]
    #[serde(default, rename(deserialize = "type"))]
    pub market_type: Option<String>,
}
