use tradingview_rs::user::LoginUserResponse;

fn main() {
    let contents = std::fs::read_to_string(
        "/home/datnguyen/Projects/tradingview-rs/tests/_data/user-login.json",
    )
    .expect("Should have been able to read the file");
    let deserialized: LoginUserResponse = serde_json::from_str(&contents)
        .expect("Failed to deserialize JSON data to TradingView struct");

    println!("{:#?}", deserialized);
}
