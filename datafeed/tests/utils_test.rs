#[cfg(test)]
mod utils {
    use datafeed::utils::*;
    #[test]
    fn parse_packet_test() {
        let current_dir = std::env::current_dir().unwrap().display().to_string();
        println!("Current dir: {}", current_dir);
        let messages =
            std::fs::read_to_string(format!("{}/tests/data/socket_messages.txt", current_dir))
                .unwrap();
        let data = protocol::parse_packet(messages.as_str());
        assert_eq!(data.len(), 42);
    }

    #[test]
    fn gen_session_id_test() {
        let quote_session = gen_session_id("qc");
        assert!(!quote_session.is_empty() && quote_session.starts_with("qc"));
    }
}
