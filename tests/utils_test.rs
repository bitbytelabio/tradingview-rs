#[cfg(test)]
mod utils {
    use tradingview_rs::utils::*;
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
}
