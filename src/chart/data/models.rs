use crate::{DataPoint, chart::ChartOptions};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ChartHistoricalData {
    pub options: ChartOptions,
    pub data: Vec<DataPoint>,
}

impl ChartHistoricalData {
    pub fn new(opt: &ChartOptions) -> Self {
        Self {
            options: opt.clone(),
            data: Vec::new(),
        }
    }
}
