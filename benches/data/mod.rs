use rand::Rng;
use tradingview_rs::chart::{ChartDataResponse, SeriesDataPoint};

pub fn generate_chart_random_data(length: u64) -> ChartDataResponse {
    let mut series = Vec::new();
    let mut rng = rand::thread_rng();

    for i in 1..=length {
        let value = (
            rng.gen_range(1.0..=10.0),
            rng.gen_range(2.0..=11.0),
            rng.gen_range(3.0..=12.0),
            rng.gen_range(4.0..=13.0),
            rng.gen_range(5.0..=14.0),
            rng.gen_range(6.0..=15.0),
        );
        let data_point = SeriesDataPoint {
            index: i,
            value: value,
        };
        series.push(data_point);
    }

    ChartDataResponse {
        node: None,
        series: series,
    }
}
