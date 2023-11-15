use crate::chart::models::{ ChartResponseData, StudyResponseData };
use serde_json::Value;

/// Extracts OHLCV data from a `ChartResponseData` object and returns a vector of `OHLCV` structs.
///
/// # Arguments
///
/// * `data` - A reference to a `ChartResponseData` object containing the series data.
///
/// # Returns
///
/// A vector of `OHLCV` structs containing the extracted data.
pub fn extract_ohlcv_data(data: ChartResponseData) -> Vec<Vec<f64>> {
    data.series
        .into_iter()
        .map(|point| point.value)
        .collect()
}

/// Extracts the data from a `StudyResponseData` object and returns it as a vector of vectors.
///
/// # Arguments
///
/// * `data` - A reference to a `StudyResponseData` object.
///
/// # Returns
///
/// A vector of vectors containing the data from the `StudyResponseData` object.
pub fn extract_studies_data(data: StudyResponseData) -> Vec<Vec<f64>> {
    data.studies
        .into_iter()
        .map(|point| point.value)
        .collect()
}

/// Returns the string value at the given index in the provided slice of `Value`s.
/// If the value at the given index is not a string, an empty string is returned.
pub fn get_string_value(values: &[Value], index: usize) -> String {
    match values.get(index) {
        Some(value) =>
            match value.as_str() {
                Some(string_value) => string_value.to_string(),
                None => "".to_string(),
            }
        None => "".to_string(),
    }
}
