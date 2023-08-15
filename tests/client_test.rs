#[cfg(test)]
mod tests {
    use tradingview_rs::client::mics::*;
    use tradingview_rs::models::*;

    #[tokio::test]
    async fn test_get_builtin_indicators() {
        let indicators = get_builtin_indicators(pine_indicator::BuiltinIndicators::All).await;
        assert!(indicators.is_ok());
        assert!(indicators.unwrap().len() > 0);
    }
}
