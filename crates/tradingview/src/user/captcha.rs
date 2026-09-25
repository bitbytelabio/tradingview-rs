use crate::{Result, error::Error};
use serde::{Deserialize, Serialize};
use std::{borrow::Cow, fmt, time::Duration};

const DEFAULT_BASE_URL: &str = "https://api.2captcha.com";
const DEFAULT_POLL_INTERVAL: Duration = Duration::from_secs(5);
const DEFAULT_TOTAL_TIMEOUT: Duration = Duration::from_secs(120);
const HTTP_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

const TRADINGVIEW_WEBSITE_URL: &str = "https://www.tradingview.com/";
const TRADINGVIEW_SITE_KEY: &str = "6Lcqv24UAAAAAIvkElDvwPxD0R8scDnMpizaBcHQ";
const RECAPTCHA_TASK_TYPE: &str = "RecaptchaV2TaskProxyless";
const RECAPTCHA_API_DOMAIN: &str = "recaptcha.net";

/// Solved CAPTCHA result holding the task ID and solution token.
#[derive(Clone, PartialEq, Eq)]
pub(super) struct SolvedCaptcha {
    pub(super) task_id: u64,
    pub(super) token: String,
}

impl fmt::Debug for SolvedCaptcha {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SolvedCaptcha")
            .field("task_id", &self.task_id)
            .field("token", &"[REDACTED]")
            .finish()
    }
}

/// 2Captcha client for solving reCAPTCHA challenges.
pub(super) struct TwoCaptcha<'a> {
    api_key: &'a str,
    base_url: Cow<'a, str>,
    poll_interval: Duration,
    timeout: Duration,
    http_timeout: Duration,
    client: wreq::Client,
}

impl<'a> fmt::Debug for TwoCaptcha<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TwoCaptcha")
            .field("api_key", &"[REDACTED]")
            .field("base_url", &self.base_url)
            .field("poll_interval", &self.poll_interval)
            .field("timeout", &self.timeout)
            .finish()
    }
}

impl<'a> TwoCaptcha<'a> {
    /// Creates a new `TwoCaptcha` solver with production defaults.
    pub(super) fn new(api_key: &'a str) -> Self {
        let client = wreq::Client::builder()
            .timeout(HTTP_REQUEST_TIMEOUT)
            .build()
            .expect("Failed to build 2Captcha HTTP client");

        Self {
            api_key,
            base_url: Cow::Borrowed(DEFAULT_BASE_URL),
            poll_interval: DEFAULT_POLL_INTERVAL,
            timeout: DEFAULT_TOTAL_TIMEOUT,
            http_timeout: HTTP_REQUEST_TIMEOUT,
            client,
        }
    }

    /// Creates a test instance with configurable base URL, polling interval, and timeouts.
    #[cfg(test)]
    pub(super) fn for_test(
        api_key: &'a str,
        base_url: &str,
        poll_interval: Duration,
        timeout: Duration,
    ) -> Self {
        let http_timeout = if timeout < HTTP_REQUEST_TIMEOUT {
            timeout
        } else {
            HTTP_REQUEST_TIMEOUT
        };

        let client = wreq::Client::builder()
            .timeout(http_timeout)
            .build()
            .expect("Failed to build test 2Captcha HTTP client");

        Self {
            api_key,
            base_url: Cow::Owned(base_url.to_string()),
            poll_interval,
            timeout,
            http_timeout,
            client,
        }
    }

    /// Solves a TradingView reCAPTCHA v2 challenge.
    pub(super) async fn solve(&self) -> Result<SolvedCaptcha> {
        if self.api_key.trim().is_empty() {
            return Err(Error::Request("2Captcha API key is empty".into()));
        }

        let solve_fut = async {
            let task_id = self.create_task().await?;
            loop {
                tokio::time::sleep(self.poll_interval).await;
                match self.get_task_result(task_id).await? {
                    PollStatus::Processing => continue,
                    PollStatus::Ready(token) => return Ok(SolvedCaptcha { task_id, token }),
                }
            }
        };

        match tokio::time::timeout(self.timeout, solve_fut).await {
            Ok(result) => result,
            Err(_) => Err(Error::Timeout("2Captcha solve timed out".into())),
        }
    }

    /// Reports an incorrectly solved captcha token back to 2Captcha for review.
    pub(super) async fn report_incorrect(&self, task_id: u64) -> Result<()> {
        if self.api_key.trim().is_empty() {
            return Err(Error::Request("2Captcha API key is empty".into()));
        }

        let url = format!("{}/reportIncorrect", self.base_url.trim_end_matches('/'));
        let payload = ReportIncorrectRequest {
            client_key: self.api_key,
            task_id,
        };

        let report_fut = async {
            let resp = self
                .client
                .post(&url)
                .json(&payload)
                .send()
                .await
                .map_err(map_wreq_error)?;

            let status = resp.status();
            if status == wreq::StatusCode::TOO_MANY_REQUESTS {
                return Err(Error::RateLimited("2Captcha rate limited: HTTP 429".into()));
            }
            if !status.is_success() {
                return Err(Error::Request(
                    format!("2Captcha HTTP error {}", status.as_u16()).into(),
                ));
            }

            let bytes = resp.bytes().await.map_err(map_wreq_error)?;
            let body: ReportIncorrectResponse = serde_json::from_slice(&bytes)
                .map_err(|_| Error::Request("2Captcha malformed JSON response".into()))?;

            if body.error_id != 0 {
                return Err(format_2captcha_error(
                    body.error_id,
                    body.error_code.as_deref(),
                ));
            }

            if body.status.as_deref() != Some("success") {
                return Err(Error::Request(
                    "2Captcha report failed: unexpected status".into(),
                ));
            }

            Ok(())
        };

        match tokio::time::timeout(self.http_timeout, report_fut).await {
            Ok(res) => res,
            Err(_) => Err(Error::Timeout("2Captcha report request timed out".into())),
        }
    }

    async fn create_task(&self) -> Result<u64> {
        let url = format!("{}/createTask", self.base_url.trim_end_matches('/'));
        let payload = CreateTaskRequest {
            client_key: self.api_key,
            task: RecaptchaTask {
                task_type: RECAPTCHA_TASK_TYPE,
                website_url: TRADINGVIEW_WEBSITE_URL,
                website_key: TRADINGVIEW_SITE_KEY,
                is_invisible: false,
                api_domain: RECAPTCHA_API_DOMAIN,
            },
        };

        let resp = self
            .client
            .post(&url)
            .json(&payload)
            .send()
            .await
            .map_err(map_wreq_error)?;

        let status = resp.status();
        if status == wreq::StatusCode::TOO_MANY_REQUESTS {
            return Err(Error::RateLimited("2Captcha rate limited: HTTP 429".into()));
        }
        if !status.is_success() {
            return Err(Error::Request(
                format!("2Captcha HTTP error {}", status.as_u16()).into(),
            ));
        }

        let bytes = resp.bytes().await.map_err(map_wreq_error)?;
        let body: CreateTaskResponse = serde_json::from_slice(&bytes)
            .map_err(|_| Error::Request("2Captcha malformed JSON response".into()))?;

        if body.error_id != 0 {
            return Err(format_2captcha_error(
                body.error_id,
                body.error_code.as_deref(),
            ));
        }

        match body.task_id {
            Some(id) if id > 0 => Ok(id),
            _ => Err(Error::Request("2Captcha response missing task id".into())),
        }
    }

    async fn get_task_result(&self, task_id: u64) -> Result<PollStatus> {
        let url = format!("{}/getTaskResult", self.base_url.trim_end_matches('/'));
        let payload = GetTaskResultRequest {
            client_key: self.api_key,
            task_id,
        };

        let resp = self
            .client
            .post(&url)
            .json(&payload)
            .send()
            .await
            .map_err(map_wreq_error)?;

        let status = resp.status();
        if status == wreq::StatusCode::TOO_MANY_REQUESTS {
            return Err(Error::RateLimited("2Captcha rate limited: HTTP 429".into()));
        }
        if !status.is_success() {
            return Err(Error::Request(
                format!("2Captcha HTTP error {}", status.as_u16()).into(),
            ));
        }

        let bytes = resp.bytes().await.map_err(map_wreq_error)?;
        let body: GetTaskResultResponse = serde_json::from_slice(&bytes)
            .map_err(|_| Error::Request("2Captcha malformed JSON response".into()))?;

        if body.error_id != 0 {
            return Err(format_2captcha_error(
                body.error_id,
                body.error_code.as_deref(),
            ));
        }

        match body.status.as_deref() {
            Some("processing") => Ok(PollStatus::Processing),
            Some("ready") => {
                let Some(solution) = body.solution else {
                    return Err(Error::Request(
                        "2Captcha response missing solution token".into(),
                    ));
                };
                let token = solution.g_recaptcha_response;
                if token.trim().is_empty() {
                    return Err(Error::Request(
                        "2Captcha response missing solution token".into(),
                    ));
                }
                Ok(PollStatus::Ready(token))
            }
            _ => Err(Error::Request("2Captcha unknown task status".into())),
        }
    }
}

enum PollStatus {
    Processing,
    Ready(String),
}

fn map_wreq_error(err: wreq::Error) -> Error {
    if err.is_timeout() {
        Error::Timeout("2Captcha request timeout".into())
    } else {
        Error::Request("2Captcha network request failed".into())
    }
}

fn sanitize_error_code(code: &str) -> Option<&'static str> {
    match code {
        "ERROR_KEY_DOES_NOT_EXIST" => Some("ERROR_KEY_DOES_NOT_EXIST"),
        "ERROR_WRONG_USER_KEY" => Some("ERROR_WRONG_USER_KEY"),
        "WRONG_USER_KEY" => Some("WRONG_USER_KEY"),
        "ERROR_NO_SLOT_AVAILABLE" => Some("ERROR_NO_SLOT_AVAILABLE"),
        "ERROR_ZERO_CAPTCHA_FILESIZE" => Some("ERROR_ZERO_CAPTCHA_FILESIZE"),
        "ERROR_TOO_BIG_CAPTCHA_FILESIZE" => Some("ERROR_TOO_BIG_CAPTCHA_FILESIZE"),
        "ERROR_PAGEURL" => Some("ERROR_PAGEURL"),
        "ERROR_ZERO_BALANCE" => Some("ERROR_ZERO_BALANCE"),
        "ERROR_IP_NOT_ALLOWED" => Some("ERROR_IP_NOT_ALLOWED"),
        "ERROR_CAPTCHA_UNSOLVABLE" => Some("ERROR_CAPTCHA_UNSOLVABLE"),
        "ERROR_BAD_DUPLICATES" => Some("ERROR_BAD_DUPLICATES"),
        "ERROR_NO_SUCH_METHOD" => Some("ERROR_NO_SUCH_METHOD"),
        "ERROR_IMAGE_TYPE_NOT_SUPPORTED" => Some("ERROR_IMAGE_TYPE_NOT_SUPPORTED"),
        "ERROR_NO_SUCH_CAPCHA_ID" => Some("ERROR_NO_SUCH_CAPCHA_ID"),
        "ERROR_IP_BLOCKED" => Some("ERROR_IP_BLOCKED"),
        "ERROR_TASK_ABSENT" => Some("ERROR_TASK_ABSENT"),
        "ERROR_TASK_NOT_SUPPORTED" => Some("ERROR_TASK_NOT_SUPPORTED"),
        "ERROR_RECAPTCHA_INVALID_SITEKEY" => Some("ERROR_RECAPTCHA_INVALID_SITEKEY"),
        "ERROR_ACCOUNT_SUSPENDED" => Some("ERROR_ACCOUNT_SUSPENDED"),
        "ERROR_BAD_PROXY" => Some("ERROR_BAD_PROXY"),
        "ERROR_BAD_PARAMETERS" => Some("ERROR_BAD_PARAMETERS"),
        "ERROR_BAD_IMGINSTRUCTIONS" => Some("ERROR_BAD_IMGINSTRUCTIONS"),
        _ => None,
    }
}

fn format_2captcha_error(error_id: i64, error_code: Option<&str>) -> Error {
    let sanitized_code = error_code.and_then(sanitize_error_code);
    if error_id == 12 || sanitized_code == Some("ERROR_CAPTCHA_UNSOLVABLE") {
        return Error::Request(
            "2Captcha error 12 (ERROR_CAPTCHA_UNSOLVABLE): provider automatically refunds unsolvable tasks"
                .into(),
        );
    }
    match (error_id, sanitized_code) {
        (id, Some(code)) if id != 0 => {
            Error::Request(format!("2Captcha error {id} ({code})").into())
        }
        (id, None) if id != 0 => Error::Request(format!("2Captcha error {id}").into()),
        (0, Some(code)) => Error::Request(format!("2Captcha error ({code})").into()),
        _ => Error::Request("2Captcha service error".into()),
    }
}

#[derive(Serialize)]
struct CreateTaskRequest<'b> {
    #[serde(rename = "clientKey")]
    client_key: &'b str,
    task: RecaptchaTask<'b>,
}

#[derive(Serialize)]
struct RecaptchaTask<'b> {
    #[serde(rename = "type")]
    task_type: &'static str,
    #[serde(rename = "websiteURL")]
    website_url: &'b str,
    #[serde(rename = "websiteKey")]
    website_key: &'b str,
    #[serde(rename = "isInvisible")]
    is_invisible: bool,
    #[serde(rename = "apiDomain")]
    api_domain: &'static str,
}

#[derive(Deserialize)]
struct CreateTaskResponse {
    #[serde(rename = "errorId")]
    error_id: i64,
    #[serde(rename = "errorCode", default)]
    error_code: Option<String>,
    #[serde(rename = "taskId", default)]
    task_id: Option<u64>,
}

#[derive(Serialize)]
struct GetTaskResultRequest<'b> {
    #[serde(rename = "clientKey")]
    client_key: &'b str,
    #[serde(rename = "taskId")]
    task_id: u64,
}

#[derive(Deserialize)]
struct GetTaskResultResponse {
    #[serde(rename = "errorId")]
    error_id: i64,
    #[serde(rename = "errorCode", default)]
    error_code: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    solution: Option<TaskSolution>,
}

#[derive(Deserialize)]
struct TaskSolution {
    #[serde(rename = "gRecaptchaResponse")]
    g_recaptcha_response: String,
}

#[derive(Serialize)]
struct ReportIncorrectRequest<'b> {
    #[serde(rename = "clientKey")]
    client_key: &'b str,
    #[serde(rename = "taskId")]
    task_id: u64,
}

#[derive(Deserialize)]
struct ReportIncorrectResponse {
    #[serde(rename = "errorId")]
    error_id: i64,
    #[serde(rename = "errorCode", default)]
    error_code: Option<String>,
    #[serde(default)]
    status: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
        sync::oneshot,
    };

    struct MockRequest {
        path: String,
        body: Vec<u8>,
    }

    struct MockResponse {
        status: u16,
        body: Vec<u8>,
        delay: Option<Duration>,
    }

    impl MockResponse {
        fn json(status: u16, body: impl Into<String>) -> Self {
            Self {
                status,
                body: body.into().into_bytes(),
                delay: None,
            }
        }

        fn delayed(status: u16, body: impl Into<String>, delay: Duration) -> Self {
            Self {
                status,
                body: body.into().into_bytes(),
                delay: Some(delay),
            }
        }
    }

    async fn spawn_mock_server<F>(handler: F) -> (String, oneshot::Sender<()>)
    where
        F: Fn(MockRequest) -> MockResponse + Send + Sync + 'static,
    {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind ephemeral port");
        let port = listener.local_addr().expect("local addr").port();
        let base_url = format!("http://127.0.0.1:{port}");
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel();
        let handler = Arc::new(handler);

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = &mut shutdown_rx => break,
                    res = listener.accept() => {
                        let Ok((mut socket, _)) = res else { break };
                        let handler = Arc::clone(&handler);
                        tokio::spawn(async move {
                            let mut buf = vec![0u8; 8192];
                            let mut n = 0;
                            while n < buf.len() {
                                let Ok(read_bytes) = socket.read(&mut buf[n..]).await else { break };
                                if read_bytes == 0 { break; }
                                n += read_bytes;
                                if buf[..n].windows(4).any(|w| w == b"\r\n\r\n") {
                                    break;
                                }
                            }
                            if n == 0 { return; }
                            let header_end = buf[..n].windows(4).position(|w| w == b"\r\n\r\n").unwrap_or(n);
                            let header_str = String::from_utf8_lossy(&buf[..header_end]);
                            let mut lines = header_str.lines();
                            let first_line = lines.next().unwrap_or("");
                            let mut parts = first_line.split_whitespace();
                            let _method = parts.next().unwrap_or("GET");
                            let path = parts.next().unwrap_or("/").to_string();

                            let mut content_length = 0;
                            for line in lines {
                                if let Some((k, v)) = line.split_once(':')
                                    && k.trim().eq_ignore_ascii_case("content-length")
                                {
                                    content_length = v.trim().parse::<usize>().unwrap_or(0);
                                }
                            }

                            let mut body = buf[header_end + 4..n].to_vec();
                            while body.len() < content_length {
                                let mut extra = vec![0u8; content_length - body.len()];
                                let Ok(read_bytes) = socket.read(&mut extra).await else { break };
                                if read_bytes == 0 { break; }
                                body.extend_from_slice(&extra[..read_bytes]);
                            }

                            let req = MockRequest { path, body };
                            let resp = handler(req);

                            if let Some(delay) = resp.delay {
                                tokio::time::sleep(delay).await;
                            }

                            let reason = match resp.status {
                                200 => "OK",
                                400 => "Bad Request",
                                429 => "Too Many Requests",
                                500 => "Internal Server Error",
                                _ => "Unknown",
                            };

                            let response_bytes = format!(
                                "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                resp.status,
                                reason,
                                resp.body.len()
                            );

                            let _ = socket.write_all(response_bytes.as_bytes()).await;
                            let _ = socket.write_all(&resp.body).await;
                            let _ = socket.flush().await;
                        });
                    }
                }
            }
        });

        (base_url, shutdown_tx)
    }

    #[tokio::test]
    async fn test_solve_processing_to_ready_token() {
        let poll_count = Arc::new(AtomicUsize::new(0));
        let poll_count_clone = Arc::clone(&poll_count);

        let (base_url, _shutdown) = spawn_mock_server(move |req| {
            if req.path == "/createTask" {
                let body: serde_json::Value = serde_json::from_slice(&req.body).unwrap();
                assert_eq!(body["clientKey"], "test_key");
                assert_eq!(body["task"]["type"], "RecaptchaV2TaskProxyless");
                assert_eq!(body["task"]["websiteURL"], "https://www.tradingview.com/");
                assert_eq!(body["task"]["websiteKey"], "6Lcqv24UAAAAAIvkElDvwPxD0R8scDnMpizaBcHQ");
                assert_eq!(body["task"]["apiDomain"], "recaptcha.net");
                MockResponse::json(200, r#"{"errorId":0,"taskId":98765}"#)
            } else if req.path == "/getTaskResult" {
                let body: serde_json::Value = serde_json::from_slice(&req.body).unwrap();
                assert_eq!(body["clientKey"], "test_key");
                assert_eq!(body["taskId"], 98765);
                let count = poll_count_clone.fetch_add(1, Ordering::SeqCst);
                if count < 2 {
                    MockResponse::json(200, r#"{"errorId":0,"status":"processing"}"#)
                } else {
                    MockResponse::json(
                        200,
                        r#"{"errorId":0,"status":"ready","solution":{"gRecaptchaResponse":"valid_token_xyz"}}"#,
                    )
                }
            } else {
                panic!("unexpected path: {}", req.path);
            }
        })
        .await;

        let solver = TwoCaptcha::for_test(
            "test_key",
            &base_url,
            Duration::from_millis(10),
            Duration::from_secs(5),
        );
        let solved = solver.solve().await.expect("solve should succeed");
        assert_eq!(solved.task_id, 98765);
        assert_eq!(solved.token, "valid_token_xyz");
        assert_eq!(poll_count.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_solve_service_error_on_create_task() {
        let (base_url, _shutdown) = spawn_mock_server(|req| {
            assert_eq!(req.path, "/createTask");
            MockResponse::json(
                200,
                r#"{"errorId":1,"errorCode":"ERROR_KEY_DOES_NOT_EXIST","errorDescription":"API key not found"}"#,
            )
        })
        .await;

        let solver = TwoCaptcha::for_test(
            "bad_key",
            &base_url,
            Duration::from_millis(10),
            Duration::from_secs(5),
        );
        let err = solver
            .solve()
            .await
            .expect_err("should fail with service error");
        match err {
            Error::Request(msg) => {
                let s = msg.as_str();
                assert!(s.contains("ERROR_KEY_DOES_NOT_EXIST"));
                assert!(!s.contains("API key not found")); // description sanitized out
            }
            other => panic!("expected Error::Request, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_solve_service_error_on_get_task_result() {
        let (base_url, _shutdown) = spawn_mock_server(|req| {
            if req.path == "/createTask" {
                MockResponse::json(200, r#"{"errorId":0,"taskId":12345}"#)
            } else {
                MockResponse::json(
                    200,
                    r#"{"errorId":10,"errorCode":"ERROR_ZERO_BALANCE","errorDescription":"Out of funds"}"#,
                )
            }
        })
        .await;

        let solver = TwoCaptcha::for_test(
            "key",
            &base_url,
            Duration::from_millis(10),
            Duration::from_secs(5),
        );
        let err = solver
            .solve()
            .await
            .expect_err("should fail with zero balance");
        match err {
            Error::Request(msg) => {
                let s = msg.as_str();
                assert!(s.contains("ERROR_ZERO_BALANCE"));
                assert!(!s.contains("Out of funds"));
            }
            other => panic!("expected Error::Request, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_solve_error_12_unsolvable_automatic_refund() {
        let (base_url, _shutdown) = spawn_mock_server(|req| {
            if req.path == "/createTask" {
                MockResponse::json(200, r#"{"errorId":0,"taskId":55555}"#)
            } else {
                MockResponse::json(
                    200,
                    r#"{"errorId":12,"errorCode":"ERROR_CAPTCHA_UNSOLVABLE","errorDescription":"Workers failed"}"#,
                )
            }
        })
        .await;

        let solver = TwoCaptcha::for_test(
            "key",
            &base_url,
            Duration::from_millis(10),
            Duration::from_secs(5),
        );
        let err = solver
            .solve()
            .await
            .expect_err("should fail with unsolvable");
        match err {
            Error::Request(msg) => {
                let s = msg.as_str();
                assert!(s.contains("ERROR_CAPTCHA_UNSOLVABLE"));
                assert!(s.contains("provider automatically refunds unsolvable tasks"));
            }
            other => panic!("expected Error::Request, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_solve_missing_task_id() {
        let (base_url, _shutdown) = spawn_mock_server(|req| {
            assert_eq!(req.path, "/createTask");
            MockResponse::json(200, r#"{"errorId":0}"#)
        })
        .await;

        let solver = TwoCaptcha::for_test(
            "key",
            &base_url,
            Duration::from_millis(10),
            Duration::from_secs(5),
        );
        let err = solver
            .solve()
            .await
            .expect_err("missing task id should fail");
        assert!(matches!(err, Error::Request(_)));
    }

    #[tokio::test]
    async fn test_solve_missing_solution() {
        let (base_url, _shutdown) = spawn_mock_server(|req| {
            if req.path == "/createTask" {
                MockResponse::json(200, r#"{"errorId":0,"taskId":22222}"#)
            } else {
                MockResponse::json(200, r#"{"errorId":0,"status":"ready"}"#)
            }
        })
        .await;

        let solver = TwoCaptcha::for_test(
            "key",
            &base_url,
            Duration::from_millis(10),
            Duration::from_secs(5),
        );
        let err = solver
            .solve()
            .await
            .expect_err("missing solution should fail");
        assert!(matches!(err, Error::Request(_)));
    }

    #[tokio::test]
    async fn test_solve_empty_token() {
        let (base_url, _shutdown) = spawn_mock_server(|req| {
            if req.path == "/createTask" {
                MockResponse::json(200, r#"{"errorId":0,"taskId":33333}"#)
            } else {
                MockResponse::json(
                    200,
                    r#"{"errorId":0,"status":"ready","solution":{"gRecaptchaResponse":"   "}}"#,
                )
            }
        })
        .await;

        let solver = TwoCaptcha::for_test(
            "key",
            &base_url,
            Duration::from_millis(10),
            Duration::from_secs(5),
        );
        let err = solver.solve().await.expect_err("empty token should fail");
        assert!(matches!(err, Error::Request(_)));
    }

    #[tokio::test]
    async fn test_solve_unknown_status() {
        let (base_url, _shutdown) = spawn_mock_server(|req| {
            if req.path == "/createTask" {
                MockResponse::json(200, r#"{"errorId":0,"taskId":44444}"#)
            } else {
                MockResponse::json(200, r#"{"errorId":0,"status":"weird_status"}"#)
            }
        })
        .await;

        let solver = TwoCaptcha::for_test(
            "key",
            &base_url,
            Duration::from_millis(10),
            Duration::from_secs(5),
        );
        let err = solver
            .solve()
            .await
            .expect_err("unknown status should fail");
        assert!(matches!(err, Error::Request(_)));
    }

    #[tokio::test]
    async fn test_solve_missing_error_id_malformed() {
        let (base_url, _shutdown) =
            spawn_mock_server(|_| MockResponse::json(200, r#"{"status":"ready"}"#)).await;

        let solver = TwoCaptcha::for_test(
            "key",
            &base_url,
            Duration::from_millis(10),
            Duration::from_secs(5),
        );
        let err = solver
            .solve()
            .await
            .expect_err("missing errorId should fail as malformed JSON");
        assert!(matches!(err, Error::Request(_)));
    }

    #[tokio::test]
    async fn test_solve_malformed_json() {
        let (base_url, _shutdown) =
            spawn_mock_server(|_| MockResponse::json(200, "{ invalid json")).await;

        let solver = TwoCaptcha::for_test(
            "key",
            &base_url,
            Duration::from_millis(10),
            Duration::from_secs(5),
        );
        let err = solver
            .solve()
            .await
            .expect_err("malformed JSON should fail");
        assert!(matches!(err, Error::Request(_)));
    }

    #[tokio::test]
    async fn test_solve_http_failure() {
        let (base_url, _shutdown) = spawn_mock_server(|_| {
            MockResponse::json(500, r#"{"errorId":500,"errorDescription":"Server error"}"#)
        })
        .await;

        let solver = TwoCaptcha::for_test(
            "key",
            &base_url,
            Duration::from_millis(10),
            Duration::from_secs(5),
        );
        let err = solver.solve().await.expect_err("HTTP 500 should fail");
        assert!(matches!(err, Error::Request(_)));
    }

    #[tokio::test]
    async fn test_solve_rate_limited() {
        let (base_url, _shutdown) =
            spawn_mock_server(|_| MockResponse::json(429, r#"{"errorId":429}"#)).await;

        let solver = TwoCaptcha::for_test(
            "key",
            &base_url,
            Duration::from_millis(10),
            Duration::from_secs(5),
        );
        let err = solver.solve().await.expect_err("HTTP 429 should fail");
        assert!(matches!(err, Error::RateLimited(_)));
    }

    #[tokio::test]
    async fn test_solve_timeout() {
        let (base_url, _shutdown) = spawn_mock_server(|req| {
            if req.path == "/createTask" {
                MockResponse::json(200, r#"{"errorId":0,"taskId":77777}"#)
            } else {
                MockResponse::json(200, r#"{"errorId":0,"status":"processing"}"#)
            }
        })
        .await;

        let solver = TwoCaptcha::for_test(
            "key",
            &base_url,
            Duration::from_millis(20),
            Duration::from_millis(50),
        );
        let err = solver.solve().await.expect_err("should time out");
        assert!(matches!(err, Error::Timeout(_)));
    }

    #[tokio::test]
    async fn test_solve_one_create_request_only() {
        let create_task_count = Arc::new(AtomicUsize::new(0));
        let create_task_count_clone = Arc::clone(&create_task_count);

        let (base_url, _shutdown) = spawn_mock_server(move |req| {
            if req.path == "/createTask" {
                create_task_count_clone.fetch_add(1, Ordering::SeqCst);
                MockResponse::json(200, r#"{"errorId":0,"taskId":88888}"#)
            } else if req.path == "/getTaskResult" {
                MockResponse::json(
                    200,
                    r#"{"errorId":0,"status":"ready","solution":{"gRecaptchaResponse":"token123"}}"#,
                )
            } else {
                panic!("unexpected path: {}", req.path);
            }
        })
        .await;

        let solver = TwoCaptcha::for_test(
            "key",
            &base_url,
            Duration::from_millis(10),
            Duration::from_secs(5),
        );
        let _ = solver.solve().await.expect("solve should succeed");
        assert_eq!(create_task_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_report_incorrect_success() {
        let (base_url, _shutdown) = spawn_mock_server(|req| {
            assert_eq!(req.path, "/reportIncorrect");
            let body: serde_json::Value = serde_json::from_slice(&req.body).unwrap();
            assert_eq!(body["clientKey"], "key_abc");
            assert_eq!(body["taskId"], 123456);
            MockResponse::json(200, r#"{"errorId":0,"status":"success"}"#)
        })
        .await;

        let solver = TwoCaptcha::for_test(
            "key_abc",
            &base_url,
            Duration::from_millis(10),
            Duration::from_secs(5),
        );
        let result = solver.report_incorrect(123456).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_report_incorrect_api_error() {
        let (base_url, _shutdown) = spawn_mock_server(|req| {
            assert_eq!(req.path, "/reportIncorrect");
            MockResponse::json(
                200,
                r#"{"errorId":16,"errorCode":"ERROR_NO_SUCH_CAPCHA_ID","errorDescription":"Unknown id"}"#,
            )
        })
        .await;

        let solver = TwoCaptcha::for_test(
            "key_abc",
            &base_url,
            Duration::from_millis(10),
            Duration::from_secs(5),
        );
        let err = solver
            .report_incorrect(99999)
            .await
            .expect_err("should return error");
        match err {
            Error::Request(msg) => {
                let s = msg.as_str();
                assert!(s.contains("ERROR_NO_SUCH_CAPCHA_ID"));
                assert!(!s.contains("Unknown id"));
            }
            other => panic!("expected Error::Request, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_report_incorrect_malformed_status() {
        let (base_url, _shutdown) = spawn_mock_server(|req| {
            assert_eq!(req.path, "/reportIncorrect");
            MockResponse::json(200, r#"{"errorId":0,"status":"declined"}"#)
        })
        .await;

        let solver = TwoCaptcha::for_test(
            "key_abc",
            &base_url,
            Duration::from_millis(10),
            Duration::from_secs(5),
        );
        let err = solver
            .report_incorrect(123456)
            .await
            .expect_err("unexpected status should fail");
        assert!(matches!(err, Error::Request(_)));
    }

    #[tokio::test]
    async fn test_report_incorrect_timeout() {
        let request_count = Arc::new(AtomicUsize::new(0));
        let request_count_clone = Arc::clone(&request_count);

        let (base_url, _shutdown) = spawn_mock_server(move |req| {
            assert_eq!(req.path, "/reportIncorrect");
            request_count_clone.fetch_add(1, Ordering::SeqCst);
            MockResponse::delayed(
                200,
                r#"{"errorId":0,"status":"success"}"#,
                Duration::from_millis(200),
            )
        })
        .await;

        let solver = TwoCaptcha::for_test(
            "key_abc",
            &base_url,
            Duration::from_millis(10),
            Duration::from_millis(50),
        );
        let err = solver
            .report_incorrect(123456)
            .await
            .expect_err("should time out");
        assert!(matches!(err, Error::Timeout(_)));
        assert_eq!(request_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_debug_redacts_sensitive_keys() {
        let solver = TwoCaptcha::new("super_secret_key_12345");
        let debug_str = format!("{solver:?}");
        assert!(!debug_str.contains("super_secret_key_12345"));
        assert!(debug_str.contains("[REDACTED]"));

        let solved = SolvedCaptcha {
            task_id: 12345,
            token: "super_secret_token_abcde".to_string(),
        };
        let solved_debug = format!("{solved:?}");
        assert!(!solved_debug.contains("super_secret_token_abcde"));
        assert!(solved_debug.contains("12345"));
        assert!(solved_debug.contains("[REDACTED]"));
    }
}
