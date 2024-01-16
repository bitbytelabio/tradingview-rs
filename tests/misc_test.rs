#[cfg(test)]
mod tests {
    use tradingview::*;

    #[tokio::test]
    async fn test_search_symbol() {
        let res = search_symbol("", "", &SymbolMarketType::Crypto, 0, "", "")
            .await
            .unwrap();

        println!("{:#?}", res);
        assert!(!res.symbols.is_empty());
    }

    #[tokio::test]
    async fn test_list_symbol() {
        let res = list_symbols(None, None, None, None).await.unwrap();

        println!("{:#?}", res.len());
        assert!(!res.is_empty());
    }

    #[tokio::test]
    async fn test_get_builtin_indicators() {
        let indicators = get_builtin_indicators(pine_indicator::BuiltinIndicators::All)
            .await
            .unwrap();
        println!("{:#?}", indicators);
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
        println!("{:#?}", metadata);
        assert_eq!(
            metadata.data.id,
            "Script$STD;Candlestick%1Pattern%1Bullish%1Upside%1Tasuki%1Gap@tv-scripting-101"
        );
    }
}
