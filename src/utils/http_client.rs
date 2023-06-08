pub use http_client::get;

const HTTP_HEADERS: vec![] = vec![
    ("Accept", "application/json, text/plain, */*"),
    ("Accept-Encoding", "gzip, deflate, br"),
    ("Accept-Language", "en-US,en;q=0.9"),
    ("Connection", "keep-alive"),
    ("Origin", "https://www.tradingview.com"),
    ("Referer", "https://www.tradingview.com/"),
    ("Sec-Fetch-Dest", "empty"),
    ("Sec-Fetch-Mode", "cors"),
    ("Sec-Fetch-Site", "same-site"),
    ("User-Agent", "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/112.0.0.0 Safari/537.36 uacq"),
];

mod http_client {
    pub async fn get(url: &str) -> Result<serde_json::Value, Error> {
        let client = reqwest::Client::new();
        client
            .get(url)
            .headers(HTTP_HEADERS)
            .send()
            .await?
            .json()
            .await;
        Ok(data)
    }
}
