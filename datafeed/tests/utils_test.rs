#[cfg(test)]

mod utils {
    use datafeed::utils::*;
    #[test]
    fn parse_packet_test() {
        let data = protocol::parse_packet(messages).unwrap();
        assert!(!data.is_empty() && data.len() == 128);
    }
}