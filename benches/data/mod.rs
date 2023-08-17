use rand::Rng;
use tradingview_rs::chart::{ChartResponseData, DataPoint};

pub fn generate_chart_random_data(length: u64) -> ChartResponseData {
    let mut series = Vec::new();
    let mut rng = rand::thread_rng();

    for _ in 1..=length {
        let value = (
            rng.gen_range(1.0..=10.0),
            rng.gen_range(2.0..=11.0),
            rng.gen_range(3.0..=12.0),
            rng.gen_range(4.0..=13.0),
            rng.gen_range(5.0..=14.0),
            rng.gen_range(6.0..=15.0),
        );
        let data_point = DataPoint { index: 0, value };
        series.push(data_point);
    }

    ChartResponseData { node: None, series }
}
