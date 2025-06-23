use colored::*;
use std::sync::Once;
use tradingview::{
    Interval, OHLCV,
    chart::{ChartOptions, fetch_chart_data},
    socket::DataServer,
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
    init();
    dotenv::dotenv().ok();

    // Print a colored header
    println!(
        "{}",
        "📈 TradingView Historical Data Fetcher 📉"
            .bright_green()
            .bold()
    );

    let auth_token = std::env::var("TV_AUTH_TOKEN").expect("TV_AUTH_TOKEN is not set");

    let symbol = "AAPL";
    let exchange = "NASDAQ";
    let interval = Interval::OneDay;
    let bars = 500_000;

    println!(
        "{} Fetching data for {} {} ({}), {} bars",
        "→".bright_blue().bold(),
        symbol.yellow().bold(),
        exchange.cyan(),
        format!("{:?}", interval).magenta(),
        bars.to_string().bright_blue()
    );

    let option = ChartOptions::new_with(symbol, exchange, interval).bar_count(bars);
    let data = fetch_chart_data()
        .auth_token(&auth_token)
        .options(option)
        .server(DataServer::ProData)
        .call()
        .await?;

    println!("{}", "✅ Data retrieved successfully!".green());
    println!("{}", "----------------------------------------".dimmed());

    // Print each data point with different colors
    for (i, bar) in data.data.iter().rev().enumerate() {
        println!(
            "{} {} | Open: {} | High: {} | Low: {} | Close: {} | Volume: {}",
            format!("[{}]", i).blue(),
            format!("{}", bar.datetime()).bright_yellow().bold(),
            format!("{:.2}", bar.open()).green(),
            format!("{:.2}", bar.high()).bright_green(),
            format!("{:.2}", bar.low()).red(),
            format!("{:.2}", bar.close()).bright_cyan().bold(),
            format!("{}", bar.volume()).magenta()
        );
    }

    println!("{}", "----------------------------------------".dimmed());
    println!("{}", "Done!".bright_green().bold());

    Ok(())
}
