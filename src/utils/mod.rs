pub mod client {
    use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, COOKIE, ORIGIN, REFERER};
    use reqwest::{Client, Error, Response};
    use tracing::{debug, info};

    #[tracing::instrument]
    pub async fn get_request(url: &str, cookies: Option<&str>) -> Result<Response, Error> {
        info!("Sending request to: {}", url);
        let client = Client::builder()
            .default_headers({
                let mut headers = HeaderMap::new();
                headers.insert(
                    ACCEPT,
                    HeaderValue::from_static("application/json, text/plain, */*"),
                );
                headers.insert(
                    ORIGIN,
                    HeaderValue::from_static("https://www.tradingview.com"),
                );
                headers.insert(
                    REFERER,
                    HeaderValue::from_static("https://www.tradingview.com/"),
                );
                headers
            })
            .connection_verbose(true)
            .https_only(true)
            .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/112.0.0.0 Safari/537.36 uacq")
            .gzip(true)
            .build()?;

        let mut request = client.get(url);
        request = match cookies {
            Some(cookies) => {
                let mut headers = HeaderMap::new();
                headers.insert(COOKIE, HeaderValue::from_str(cookies).unwrap());
                request.headers(headers)
            }
            None => request,
        };
        debug!("Sending request: {:?}", request);
        let response = request.send().await?;
        Ok(response)
    }
}
