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

pub fn ema(ohlcv: &[DataPoint], period: u8) -> Result<Vec<f64>> {
    if ohlcv.is_empty() {
        return Ok(Vec::new());
    }

    let mut ema = EMA::new(period, &ohlcv[0].close())?;
    let mut ema_values = Vec::with_capacity(ohlcv.len());
    for price in ohlcv.iter().skip(1) {
        let value = ema.next(&price.close());
        ema_values.push(value);
    }

    Ok(ema_values)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Interval;
    use crate::chart::ChartOptions;
    use crate::data::fetch_chart_historical;
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
        let symbol = "HOSE:FPT";
        let interval = Interval::Daily;
        let option = ChartOptions::new(symbol, interval).bar_count(10_000);
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
        let symbol = "HOSE:FPT";
        let interval = Interval::Daily;
        let option = ChartOptions::new(symbol, interval).bar_count(10_000);
        let server = Some(DataServer::ProData);
        let data = fetch_chart_historical(&auth_token, option, server).await?;

        println!("Fetched data: {:?}", data.data.len());
        let result = ema(&data.data, 14)?;
        println!("EMA: {:?}", result);
        Ok(())
    }
}
