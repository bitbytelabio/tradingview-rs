use colored::*;
use std::sync::Once;
use tradingview::{
    Country, Interval, MarketType, OHLCV, StocksType, chart::ChartOptions, fetch_chart_data_batch,
    list_symbols, socket::DataServer,
};

fn init() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        tracing_subscriber::fmt()
            .with_max_level(tracing::Level::INFO)
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
        .exchange("UPCOM")
        .market_type(MarketType::Stocks(StocksType::Common))
        .country(Country::VN)
        .call()
        .await?[0..50]
        .to_vec();

    assert!(!symbols.is_empty(), "No symbols found");

    let based_opt = ChartOptions::builder()
        .interval(Interval::OneDay)
        .bar_count(10)
        .build();

    let data = fetch_chart_data_batch(
        &auth_token,
        &symbols,
        based_opt,
        Some(DataServer::ProData),
        40,
    )
    .await?;

    println!("{}", "✅ Data retrieved successfully!".green());
    println!("{}", "----------------------------------------".dimmed());

    for bar in data.values() {
        println!(
            "{} | {} | {} | {} | {}",
            format!("Symbol: {}", bar.symbol_info.name)
                .bright_cyan()
                .bold(),
            format!("Exchange: {}", bar.symbol_info.exchange).green(),
            format!("Description: {}", bar.symbol_info.description).yellow(),
            format!("Currency: {}", bar.symbol_info.currency_code).blue(),
            format!("Country: {}", bar.symbol_info.currency_code).magenta(),
        );

        for (i, ohlcv) in bar.data.iter().rev().enumerate() {
            println!(
                "{} {} | Open: {} | High: {} | Low: {} | Close: {} | Volume: {}",
                format!("[{}]", i).blue(),
                format!("{}", ohlcv.datetime()).bright_yellow().bold(),
                format!("{:.2}", ohlcv.open()).green(),
                format!("{:.2}", ohlcv.high()).bright_green(),
                format!("{:.2}", ohlcv.low()).red(),
                format!("{:.2}", ohlcv.close()).bright_cyan().bold(),
                format!("{}", ohlcv.volume()).magenta()
            );
        }
    }

    println!("{}", "----------------------------------------".dimmed());
    println!("{}", "Done!".bright_green().bold());

    Ok(())
}
