pub enum TradingViewCommand {
    Subscribe(String),
    Unsubscribe(String),
    Ping,
    Pong,
}
