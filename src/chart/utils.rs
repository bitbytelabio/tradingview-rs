use crate::{
    chart::{ChartResponseData, StudyResponseData},
    models::OHLCV,
};
use rayon::prelude::*;
use serde_json::Value;
use std::sync::Mutex;

/// Extracts OHLCV data from a `ChartResponseData` object and returns a vector of `OHLCV` structs.
///
/// # Arguments
///
/// * `data` - A reference to a `ChartResponseData` object containing the series data.
///
/// # Returns
///
/// A vector of `OHLCV` structs containing the extracted data.
pub fn extract_ohlcv_data(data: &ChartResponseData) -> Vec<OHLCV> {
    data.series
        .iter()
        .map(|point| OHLCV::new(point.value))
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
pub fn extract_studies_data(data: &StudyResponseData) -> Vec<Vec<f64>> {
    data.studies
        .iter()
        .map(|point| point.value.clone())
        .collect()
}

/// Extracts OHLCV data from a `ChartResponseData` struct in parallel.
///
/// # Arguments
///
/// * `data` - A reference to a `ChartResponseData` struct.
///
/// # Returns
///
/// A vector of `OHLCV` structs.
pub fn par_extract_ohlcv_data(data: &ChartResponseData) -> Vec<OHLCV> {
    data.series
        .par_iter()
        .map(|point| OHLCV::new(point.value))
        .collect()
}

/// Sorts an array of OHLCV tuples by their timestamp in ascending order.
///
/// # Arguments
///
/// * `tuples` - A mutable reference to an array of OHLCV tuples to be sorted.
///
pub fn sort_ohlcv_tuples(tuples: &mut [OHLCV]) {
    tuples.sort_by(|a, b| {
        a.timestamp
            .partial_cmp(&b.timestamp)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
}

/// Update an OHLCV data point in a vector of OHLCV data.
///
/// If a data point with the same timestamp as `new_data` exists in `data`, it will be updated with `new_data`.
/// Otherwise, `new_data` will be added to `data` and the vector will be sorted by timestamp.
///
/// # Arguments
///
/// * `data` - A mutable reference to a vector of OHLCV data.
/// * `new_data` - The new OHLCV data point to update or add to `data`.
pub fn update_ohlcv_data_point(data: &mut Vec<OHLCV>, new_data: OHLCV) {
    if let Some(index) = data
        .iter()
        .position(|&x| ((x.timestamp - new_data.timestamp) as f64).abs() < f64::EPSILON)
    {
        data[index] = new_data;
    } else {
        data.push(new_data);
        data.sort_by(|a, b| {
            a.timestamp
                .partial_cmp(&b.timestamp)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }
}

/// Updates the OHLCV data by merging the new data into the old data.
///
/// # Arguments
///
/// * `old_data` - A mutable reference to the old OHLCV data.
/// * `new_data` - A reference to the new OHLCV data to be merged.
///
/// # Example
///
/// ```
/// use tradingview::tools::update_ohlcv_data;
/// use tradingview::models::OHLCV;
///
/// ```
pub fn update_ohlcv_data(old_data: &mut Vec<OHLCV>, new_data: &Vec<OHLCV>) {
    let mutex = Mutex::new(old_data);
    new_data.par_iter().for_each(|op| {
        let mut data = mutex.lock().unwrap();
        update_ohlcv_data_point(&mut data, *op)
    });
}

/// Returns the string value at the given index in the provided slice of `Value`s.
/// If the value at the given index is not a string, an empty string is returned.
pub fn get_string_value(values: &[Value], index: usize) -> String {
    match values.get(index) {
        Some(value) => match value.as_str() {
            Some(string_value) => string_value.to_string(),
            None => "".to_string(),
        },
        None => "".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chart::{ChartResponseData, DataPoint};
    use serde_json::json;

    #[test]
    fn test_sort_ohlcv_tuples() {
        let mut tuples = vec![
            OHLCV::new((1691560800.0, 83800.0, 83900.0, 83000.0, 83100.0, 708100.0)),
            OHLCV::new((1691647200.0, 82600.0, 82800.0, 82200.0, 82200.0, 784500.0)),
            OHLCV::new((1691733600.0, 81600.0, 83000.0, 81500.0, 82000.0, 625500.0)),
            OHLCV::new((1691632800.0, 83100.0, 83300.0, 82300.0, 82600.0, 558800.0)),
            OHLCV::new((1691719200.0, 82000.0, 82200.0, 81600.0, 81600.0, 517400.0)),
        ];

        sort_ohlcv_tuples(&mut tuples);

        let expected_output = vec![
            OHLCV::new((1691560800.0, 83800.0, 83900.0, 83000.0, 83100.0, 708100.0)),
            OHLCV::new((1691632800.0, 83100.0, 83300.0, 82300.0, 82600.0, 558800.0)),
            OHLCV::new((1691647200.0, 82600.0, 82800.0, 82200.0, 82200.0, 784500.0)),
            OHLCV::new((1691719200.0, 82000.0, 82200.0, 81600.0, 81600.0, 517400.0)),
            OHLCV::new((1691733600.0, 81600.0, 83000.0, 81500.0, 82000.0, 625500.0)),
        ];

        assert_eq!(tuples, expected_output);
    }

    #[test]
    fn test_update_ohlcv_data_point() {
        let mut data = vec![
            OHLCV::new((1691560800.0, 83800.0, 83900.0, 83000.0, 83100.0, 708100.0)),
            OHLCV::new((1691647200.0, 82600.0, 82800.0, 82200.0, 82200.0, 784500.0)),
            OHLCV::new((1691733600.0, 81600.0, 83000.0, 81500.0, 82000.0, 625500.0)),
            OHLCV::new((1691632800.0, 83100.0, 83300.0, 82300.0, 82600.0, 558800.0)),
            OHLCV::new((1691719200.0, 82000.0, 82200.0, 81600.0, 81600.0, 517400.0)),
        ];

        let new_data = OHLCV::new((1691647200.0, 82700.0, 82900.0, 82100.0, 82300.0, 800000.0));

        update_ohlcv_data_point(&mut data, new_data);

        let expected_output = vec![
            OHLCV::new((1691560800.0, 83800.0, 83900.0, 83000.0, 83100.0, 708100.0)),
            OHLCV::new((1691647200.0, 82700.0, 82900.0, 82100.0, 82300.0, 800000.0)),
            OHLCV::new((1691733600.0, 81600.0, 83000.0, 81500.0, 82000.0, 625500.0)),
            OHLCV::new((1691632800.0, 83100.0, 83300.0, 82300.0, 82600.0, 558800.0)),
            OHLCV::new((1691719200.0, 82000.0, 82200.0, 81600.0, 81600.0, 517400.0)),
        ];

        assert_eq!(data, expected_output);
    }

    #[test]
    fn test_update_ohlcv_data() {
        let mut old_data = vec![
            OHLCV::new((1691560800.0, 83800.0, 83900.0, 83000.0, 83100.0, 708100.0)),
            OHLCV::new((1691647200.0, 82600.0, 82800.0, 82200.0, 82200.0, 784500.0)),
            OHLCV::new((1691733600.0, 81600.0, 83000.0, 81500.0, 82000.0, 625500.0)),
            OHLCV::new((1691632800.0, 83100.0, 83300.0, 82300.0, 82600.0, 558800.0)),
            OHLCV::new((1691719200.0, 82000.0, 82200.0, 81600.0, 81600.0, 517400.0)),
        ];

        let new_data = vec![
            OHLCV::new((1691647200.0, 82700.0, 82900.0, 82100.0, 82300.0, 800000.0)),
            OHLCV::new((1691733600.0, 81700.0, 83100.0, 81600.0, 83000.0, 700000.0)),
            OHLCV::new((1691632800.0, 83200.0, 83400.0, 82400.0, 82700.0, 600000.0)),
            OHLCV::new((1691805600.0, 82500.0, 82700.0, 82000.0, 82100.0, 900000.0)),
        ];

        update_ohlcv_data(&mut old_data, &new_data);

        let expected_output = vec![
            OHLCV::new((1691560800.0, 83800.0, 83900.0, 83000.0, 83100.0, 708100.0)),
            OHLCV::new((1691632800.0, 83200.0, 83400.0, 82400.0, 82700.0, 600000.0)),
            OHLCV::new((1691647200.0, 82700.0, 82900.0, 82100.0, 82300.0, 800000.0)),
            OHLCV::new((1691719200.0, 82000.0, 82200.0, 81600.0, 81600.0, 517400.0)),
            OHLCV::new((1691733600.0, 81700.0, 83100.0, 81600.0, 83000.0, 700000.0)),
            OHLCV::new((1691805600.0, 82500.0, 82700.0, 82000.0, 82100.0, 900000.0)),
        ];

        assert_eq!(old_data, expected_output, "values don't match");
    }

    #[test]
    fn test_process_chart_data() {
        let chart_data = ChartResponseData {
            node: None,
            series: vec![
                DataPoint {
                    value: (1.0, 2.0, 3.0, 4.0, 5.0, 6.0),
                    index: 1,
                },
                DataPoint {
                    index: 2,
                    value: (2.0, 3.0, 4.0, 5.0, 6.0, 7.0),
                },
                DataPoint {
                    index: 3,
                    value: (3.0, 4.0, 5.0, 6.0, 7.0, 8.0),
                },
                DataPoint {
                    index: 4,
                    value: (4.0, 5.0, 6.0, 7.0, 8.0, 9.0),
                },
                DataPoint {
                    index: 5,
                    value: (5.0, 6.0, 7.0, 8.0, 9.0, 10.0),
                },
                DataPoint {
                    index: 6,
                    value: (6.0, 7.0, 8.0, 9.0, 10.0, 11.0),
                },
                DataPoint {
                    index: 7,
                    value: (7.0, 8.0, 9.0, 10.0, 11.0, 12.0),
                },
                DataPoint {
                    index: 8,
                    value: (8.0, 9.0, 10.0, 11.0, 12.0, 13.0),
                },
                DataPoint {
                    index: 9,
                    value: (9.0, 10.0, 11.0, 12.0, 13.0, 14.0),
                },
                DataPoint {
                    index: 10,
                    value: (10.0, 11.0, 12.0, 13.0, 14.0, 15.0),
                },
            ],
        };
        let expected_output = vec![
            OHLCV::new((1.0, 2.0, 3.0, 4.0, 5.0, 6.0)),
            OHLCV::new((2.0, 3.0, 4.0, 5.0, 6.0, 7.0)),
            OHLCV::new((3.0, 4.0, 5.0, 6.0, 7.0, 8.0)),
            OHLCV::new((4.0, 5.0, 6.0, 7.0, 8.0, 9.0)),
            OHLCV::new((5.0, 6.0, 7.0, 8.0, 9.0, 10.0)),
            OHLCV::new((6.0, 7.0, 8.0, 9.0, 10.0, 11.0)),
            OHLCV::new((7.0, 8.0, 9.0, 10.0, 11.0, 12.0)),
            OHLCV::new((8.0, 9.0, 10.0, 11.0, 12.0, 13.0)),
            OHLCV::new((9.0, 10.0, 11.0, 12.0, 13.0, 14.0)),
            OHLCV::new((10.0, 11.0, 12.0, 13.0, 14.0, 15.0)),
        ];
        let output = extract_ohlcv_data(&chart_data);
        let output2 = par_extract_ohlcv_data(&chart_data);
        assert_eq!(output, expected_output);
        assert_eq!(output2, expected_output);
    }

    #[test]
    fn test_get_string_value() {
        let values = vec![json!("hello"), json!(null), json!(123), json!("world")];

        assert_eq!(get_string_value(&values, 0), "hello".to_string());
        assert_eq!(get_string_value(&values, 1), "".to_string());
        assert_eq!(get_string_value(&values, 2), "".to_string());
        assert_eq!(get_string_value(&values, 3), "world".to_string());
        assert_eq!(get_string_value(&values, 4), "".to_string());
    }
}
