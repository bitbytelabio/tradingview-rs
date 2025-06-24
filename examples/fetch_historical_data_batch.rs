#![allow(unused)]

use colored::*;
use std::sync::Once;
use tradingview::{
    Country, Interval, MarketType, OHLCV, StocksType, Symbol, fetch_chart_data_batch, list_symbols,
};

fn init() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::DEBUG)
            .init();
    });
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    init();
    dotenv::dotenv().ok();

    println!(
        "{}",
        "📈 TradingView Historical Data Fetcher 📉"
            .bright_green()
            .bold()
    );

    let auth_token = std::env::var("TV_AUTH_TOKEN").expect("TV_AUTH_TOKEN is not set");

    let symbols = list_symbols()
        .market_type(MarketType::Stocks(StocksType::Common))
        .country(Country::VN)
        .call()
        .await?[0..100]
        .to_vec();
    // let symbols = vec![
    //     Symbol::builder().symbol("XAUUSD").exchange("OANDA").build(),
    //     Symbol::builder()
    //         .symbol("SOLUSDT.P")
    //         .exchange("OKX")
    //         .build(),
    //     Symbol::builder()
    //         .symbol("DOGEUSDT.P")
    //         .exchange("OKX")
    //         .build(),
    //     Symbol::builder()
    //         .symbol("BNBUSDT.P")
    //         .exchange("OKX")
    //         .build(),
    // ];

    assert!(!symbols.is_empty(), "No symbols found");

    let datamap = fetch_chart_data_batch()
        .auth_token(&auth_token)
        .symbols(&symbols)
        .interval(&[Interval::OneDay, Interval::OneHour, Interval::OneWeek])
        .call()
        .await?;

    println!("{}", "✅ Data retrieved successfully!".green());
    println!("{}", "----------------------------------------".dimmed());

    for ticker_data in datamap.values() {
        println!(
            "{} | {} | {} | {} | {}",
            format!("Symbol: {}", ticker_data.symbol_info.name)
                .bright_cyan()
                .bold(),
            format!("Exchange: {}", ticker_data.symbol_info.exchange).green(),
            format!("Description: {}", ticker_data.symbol_info.description).yellow(),
            format!("Currency: {}", ticker_data.symbol_info.currency_code).blue(),
            format!("Country: {}", ticker_data.symbol_info.currency_code).magenta(),
        );

        println!("{}", "----------------------------------------".dimmed());
        let mut data = ticker_data.data.clone();
        data.dedup_by_key(|point| point.timestamp());
        data.sort_by(|a, b| a.timestamp().cmp(&b.timestamp()));
        println!(
            "{} Total data points: {}, intervals: {}",
            "📊".bright_yellow(),
            data.len().to_string().bright_blue(),
            ticker_data.series_info.options.interval
        );

        // for (i, ohlcv) in bar.data.iter().rev().enumerate() {
        //     println!(å
        //         "{} {} | Open: {} | High: {} | Low: {} | Close: {} | Volume: {}",
        //         format!("[{}]", i).blue(),
        //         format!("{}", ohlcv.datetime()).bright_yellow().bold(),
        //         format!("{:.2}", ohlcv.open()).green(),
        //         format!("{:.2}", ohlcv.high()).bright_green(),
        //         format!("{:.2}", ohlcv.low()).red(),
        //         format!("{:.2}", ohlcv.close()).bright_cyan().bold(),
        //         format!("{}", ohlcv.volume()).magenta()
        //     );
        // }
    }

    println!("{}", "----------------------------------------".dimmed());
    println!("{}", "Done!".bright_green().bold());

    Ok(())
}
