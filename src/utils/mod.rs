pub mod client {
    use reqwest::header::{
        HeaderMap, HeaderValue, ACCEPT, ACCEPT_ENCODING, ACCEPT_LANGUAGE, CONNECTION, COOKIE,
        ORIGIN, REFERER, USER_AGENT,
    };
    use reqwest::{Client, Error, Response};
    use tracing::{debug, info};

    #[tracing::instrument]
    pub async fn get_request(url: &str, cookies: Option<&str>) -> Result<Response, Error> {
        let client = Client::builder().default_headers({
            let mut headers = HeaderMap::new();
            headers.insert(
                ACCEPT,
                HeaderValue::from_static("application/json, text/plain, */*"),
            );
            headers.insert(
                ACCEPT_ENCODING,
                HeaderValue::from_static("gzip, deflate, br"),
            );
            headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("en-US,en;q=0.9"));
            headers.insert(CONNECTION, HeaderValue::from_static("keep-alive"));
            headers.insert(
                ORIGIN,
                HeaderValue::from_static("https://www.tradingview.com"),
            );
            headers.insert(
                REFERER,
                HeaderValue::from_static("https://www.tradingview.com/"),
            );
            headers.insert(USER_AGENT, HeaderValue::from_static("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/112.0.0.0 Safari/537.36 uacq"));
            headers
        })
        .https_only(true)
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

        let response = request.send().await?;
        Ok(response)
    }
}
