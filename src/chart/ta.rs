use std::f64::NAN;

use crate::{DataPoint, Result};
use yata::core::IndicatorResult;
use yata::indicators::RelativeStrengthIndex as RSI;
use yata::methods::EMA;
use yata::prelude::*;

pub fn rsi(ohlcv: &[DataPoint]) -> Result<Vec<IndicatorResult>> {
    if ohlcv.is_empty() {
        return Ok(Vec::new());
    }

    let mut rsi = RSI::default().init(&ohlcv[0])?;
    let mut rsi_values = Vec::with_capacity(ohlcv.len());
    // Process remaining prices
    for price in ohlcv.iter().skip(1) {
        let value = rsi.next(price);
        rsi_values.push(value);
    }

    Ok(rsi_values)
}

pub fn ema_recursive(close: &[f64], period: u8) -> Result<f64> {
    if close.is_empty() {
        return Ok(NAN);
    }
    let mut ema = EMA::new(period, &close[0])?;
    let mut final_ema = NAN;
    for price in close.iter().skip(1) {
        final_ema = ema.next(&price);
    }
    Ok(final_ema)
}

pub fn ema_weighted_linear(close: &[f64], _period: u8) -> Result<f64> {
    if close.is_empty() {
        return Ok(NAN);
    }
    todo!("Implement weighted linear EMA");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Interval;
    use crate::chart::ChartOptions;
    use crate::chart::data::fetch_chart_historical;
    use crate::socket::DataServer;
    use anyhow::Ok;
    use std::sync::Once;

    fn init() {
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            tracing_subscriber::fmt()
                .with_max_level(tracing::Level::DEBUG)
                .init();
        });
    }
    #[tokio::test]
    async fn test_rsi() -> anyhow::Result<()> {
        init();
        dotenv::dotenv().ok();
        let auth_token = std::env::var("TV_AUTH_TOKEN").expect("TV_AUTH_TOKEN is not set");
        let interval = Interval::Daily;
        let option = ChartOptions::new("FPT", "HOSE", interval).bar_count(30);
        let server = Some(DataServer::ProData);
        let data = fetch_chart_historical(&auth_token, option, server).await?;

        println!("Fetched data: {:?}", data.data.len());
        let result = rsi(&data.data)?;
        println!("RSI: {:?}", result);
        Ok(())
    }

    #[tokio::test]
    async fn test_ema() -> anyhow::Result<()> {
        init();
        dotenv::dotenv().ok();
        let auth_token = std::env::var("TV_AUTH_TOKEN").expect("TV_AUTH_TOKEN is not set");
        let interval = Interval::Daily;
        let option = ChartOptions::new("FPT", "HOSE", interval).bar_count(20);
        let server = Some(DataServer::ProData);
        let data = fetch_chart_historical(&auth_token, option, server).await?;
        println!("Fetched data: {:?}", data.data.len());
        let close_prices: Vec<f64> = data.data.iter().map(|x| x.close()).collect();
        println!("Close Prices: {:?}", close_prices);
        let result = ema_recursive(&close_prices, 14)?;
        println!("EMA: {:?}", result);
        Ok(())
    }
}
