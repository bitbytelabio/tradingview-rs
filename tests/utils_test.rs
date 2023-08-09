#[cfg(test)]
mod utils {
    use tradingview_rs::{utils::*, MarketAdjustment, SessionType};
    #[test]
    fn test_parse_packet() {
        let current_dir = std::env::current_dir().unwrap().display().to_string();
        println!("Current dir: {}", current_dir);
        let messages =
            std::fs::read_to_string(format!("{}/tests/data/socket_messages.txt", current_dir))
                .unwrap();
        let result = parse_packet(messages.as_str());

        assert!(result.is_ok());
        let data = result.unwrap();
        assert_eq!(data.len(), 42);
    }

    #[test]
    fn test_gen_session_id() {
        let session_type = "qc";
        let session_id = gen_session_id(session_type);
        assert_eq!(session_id.len(), 15); // 2 (session_type) + 1 (_) + 12 (random characters)
        assert!(session_id.starts_with(session_type));
    }

    #[test]
    fn test_clean_em_tags() {
        let list_text = vec![
            ("<em>AAPL</em>", "AAPL"),
            (
                "Direxion Daily <em>AAPL</em> Bear 1X Shares",
                "Direxion Daily AAPL Bear 1X Shares",
            ),
            ("<em>AAPL</em> ALPHA INDEX", "AAPL ALPHA INDEX"),
        ];
        for text in list_text {
            let cleaned_text = clean_em_tags(text.0);
            assert_eq!(cleaned_text.unwrap(), text.1);
        }
    }

    #[test]
    fn test_symbol_init() {
        let test1 = symbol_init("NSE:NIFTY", None, None, None);
        assert!(test1.is_ok());
        assert_eq!(
            test1.unwrap(),
            r#"={"adjustment":"splits","symbol":"NSE:NIFTY"}"#.to_string()
        );

        let test2 = symbol_init(
            "HOSE:FPT",
            Some(MarketAdjustment::Dividends),
            Some(iso_currency::Currency::USD),
            Some(SessionType::Extended),
        );
        assert!(test2.is_ok());
        assert_eq!(
            test2.unwrap(),
            r#"={"adjustment":"dividends","currency-id":"USD","session":"extended","symbol":"HOSE:FPT"}"#.to_string()
        );
    }
}
