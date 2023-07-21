#[cfg(test)]
mod utils {
    // #[test]
    // fn parse_packet_test() {
    //     let current_dir = std::env::current_dir().unwrap().display().to_string();
    //     println!("Current dir: {}", current_dir);
    //     let messages =
    //         std::fs::read_to_string(format!("{}/tests/data/socket_messages.txt", current_dir))
    //             .unwrap();
    //     let data = tradingview_rs::utils::parse_packet(messages.as_str());
    //     assert_eq!(data.len(), 42);
    // }

    #[test]
    fn test_gen_session_id() {
        let session_type = "qc";
        let session_id = tradingview_rs::utils::gen_session_id(session_type);
        assert_eq!(session_id.len(), 15); // 2 (session_type) + 1 (_) + 12 (random characters)
        assert!(session_id.starts_with(session_type));
    }
}
