#[cfg(test)]
mod tests {
    use tradingview::*;

    #[tokio::test]
    async fn test_search_symbol() {
        let res = advanced_search_symbol()
            .market_type(MarketType::Crypto(CryptoType::All))
            .call()
            .await
            .unwrap();

        println!("{res:#?}");
        assert!(!res.symbols.is_empty());
    }

    #[tokio::test]
    async fn test_list_symbol() {
        let res = list_symbols().call().await.unwrap();

        println!("{:#?}", res[0]);
        assert!(!res.is_empty());
    }

    #[tokio::test]
    async fn test_get_builtin_indicators() {
        let indicators = get_builtin_indicators(pine_indicator::BuiltinIndicators::All)
            .await
            .unwrap();
        println!("{indicators:#?}");
        assert!(!indicators.is_empty());
    }

    #[tokio::test]
    async fn test_get_indicator_metadata() {
        let metadata = get_indicator_metadata(
            None,
            "STD;Candlestick%1Pattern%1Bullish%1Upside%1Tasuki%1Gap",
            "19.0",
        )
        .await
        .unwrap();
        println!("{metadata:#?}");
        assert_eq!(
            metadata.data.id,
            "Script$STD;Candlestick%1Pattern%1Bullish%1Upside%1Tasuki%1Gap@tv-scripting-101"
        );
    }

    #[tokio::test]
    async fn test_get_quote_token() {
        let cookies = UserCookies::new();
        let token = get_quote_token(&cookies).await;
        // Must return error cause we are not logged in
        assert!(token.is_err());
    }
}
