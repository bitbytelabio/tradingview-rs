pub use crate::models::UserCookies;
use crate::{
    Result,
    error::{Error, LoginError},
};
use std::sync::LazyLock;
use wreq_util::{Emulation, Platform, Profile};

mod captcha;

use captcha::TwoCaptcha;

static USER_CLIENT: LazyLock<wreq::Client> = LazyLock::new(|| {
    let emulation = Emulation::builder()
        .profile(Profile::Chrome149)
        .platform(Platform::MacOS)
        .http2(true)
        .build();

    wreq::Client::builder()
        .emulation(emulation)
        .https_only(true)
        .build()
        .expect("Failed to build user HTTP client")
});

pub(crate) fn user_http_client() -> &'static wreq::Client {
    &USER_CLIENT
}
use serde::Deserialize;
use serde_json::Value;
use totp_rs::{Builder, Secret, Totp, TotpError};
use tracing::{info, warn};
use wreq::{Response, header::COOKIE};

fn get_cookie_value(jar: &wreq::cookie::Jar, uri: &str, name: &str) -> String {
    jar.matches(uri)
        .filter(|c| c.name() == name)
        .last()
        .map(|c| c.value().to_string())
        .unwrap_or_default()
}

fn is_challenge_eligible_status(status: wreq::StatusCode) -> bool {
    (status.is_success()
        || status == wreq::StatusCode::BAD_REQUEST
        || status == wreq::StatusCode::UNAUTHORIZED
        || status == wreq::StatusCode::FORBIDDEN)
        && !status.is_server_error()
}

fn is_rate_limited(status: wreq::StatusCode, body: Option<&Value>) -> bool {
    if status == wreq::StatusCode::TOO_MANY_REQUESTS {
        return true;
    }
    if let Some(body) = body {
        if body.get("code").and_then(|v| v.as_str()) == Some("rate_limit") {
            return true;
        }
        if body.get("error").and_then(|v| v.as_str()) == Some("rate_limit") {
            return true;
        }
    }
    false
}

impl UserCookies {
    pub fn new() -> Self {
        Default::default()
    }

    pub async fn login(
        &mut self,
        username: &str,
        password: &str,
        totp_secret: Option<&str>,
    ) -> Result<Self> {
        self.login_internal(
            user_http_client(),
            "https://www.tradingview.com",
            username,
            password,
            totp_secret,
        )
        .await
    }

    /// Logs in using 2Captcha to solve reCAPTCHA v2 challenges when encountered.
    ///
    /// # Semantics
    /// - Explicit opt-in only: requires a valid non-empty `api_key` string.
    /// - Performs initial signin attempt. If challenged with `recaptcha_required`:
    ///   - Dispatches at most **one** paid `createTask` solve task to 2Captcha.
    ///   - Bounded by a 120-second total solve deadline and 30-second per-request timeouts.
    ///   - Retries signin with the solution token and all retained challenge cookies.
    ///   - If the second signin attempt still requires captcha:
    ///     - Calls 2Captcha `reportIncorrect` once for provider review.
    ///     - Refund is subject to 2Captcha provider review and **not** guaranteed.
    ///     - Returns [`Error::Login`] with [`LoginError::CaptchaRequired`].
    /// - Does **not** invoke the solver on invalid credentials, HTTP 429 rate limits,
    ///   HTTP 5xx server errors, or secondary MFA challenges.
    ///
    /// # Errors
    /// Returns [`Error::Request`] if the API key is blank/whitespace or solver requests fail.
    /// Returns [`Error::RateLimited`] if rate-limited during signin or captcha retry.
    /// Returns [`Error::Login`] on invalid credentials, 2FA errors, or unsolved captchas.
    pub async fn login_with_captcha(
        &mut self,
        username: &str,
        password: &str,
        totp_secret: Option<&str>,
        api_key: &str,
    ) -> Result<Self> {
        if username.trim().is_empty() || password.trim().is_empty() {
            return Err(Error::Login {
                source: LoginError::EmptyCredentials,
            });
        }
        let trimmed_key = api_key.trim();
        if trimmed_key.is_empty() {
            return Err(Error::Request("2Captcha API key cannot be empty".into()));
        }
        let solver = TwoCaptcha::new(trimmed_key);
        self.login_orchestrated(
            user_http_client(),
            "https://www.tradingview.com",
            username,
            password,
            totp_secret,
            Some(&solver),
        )
        .await
    }

    async fn login_internal(
        &mut self,
        client: &wreq::Client,
        base_url: &str,
        username: &str,
        password: &str,
        totp_secret: Option<&str>,
    ) -> Result<Self> {
        self.login_orchestrated(client, base_url, username, password, totp_secret, None)
            .await
    }

    async fn login_orchestrated(
        &mut self,
        client: &wreq::Client,
        base_url: &str,
        username: &str,
        password: &str,
        totp_secret: Option<&str>,
        solver: Option<&TwoCaptcha<'_>>,
    ) -> Result<Self> {
        if username.trim().is_empty() || password.trim().is_empty() {
            return Err(Error::Login {
                source: LoginError::EmptyCredentials,
            });
        }

        let cookie_jar = std::sync::Arc::new(wreq::cookie::Jar::default());
        let signin_url = format!("{base_url}/accounts/signin/");
        let response = client
            .post(&signin_url)
            .cookie_provider(std::sync::Arc::clone(&cookie_jar))
            .header(wreq::header::ORIGIN, base_url)
            .header(wreq::header::REFERER, format!("{base_url}/"))
            .form(&[
                ("username", username),
                ("password", password),
                ("remember", "true"),
            ])
            .send()
            .await?;

        #[derive(Debug, Deserialize)]
        struct LoginUserResponse {
            user: UserCookies,
        }

        let status = response.status();
        if status == wreq::StatusCode::TOO_MANY_REQUESTS {
            return Err(Error::RateLimited(
                "HTTP 429 Too Many Requests during signin".into(),
            ));
        }

        let bytes = response.bytes().await?;
        let body: Value = match serde_json::from_slice(&bytes) {
            Ok(val) => val,
            Err(_) => {
                if !status.is_success() {
                    return Err(Error::Request(
                        format!("HTTP signin failed with status {status}").into(),
                    ));
                }
                return Err(Error::JsonParse(
                    "failed to parse signin JSON response".into(),
                ));
            }
        };

        if is_rate_limited(status, Some(&body)) {
            return Err(Error::RateLimited(
                "Rate limit exceeded during signin".into(),
            ));
        }

        if status.is_server_error() {
            return Err(Error::Request(
                format!("HTTP signin failed with status {status}").into(),
            ));
        }

        let (body, status) = if is_challenge_eligible_status(status) && is_recaptcha_required(&body)
        {
            let Some(solver) = solver else {
                warn!("TradingView signin required captcha");
                return Err(Error::Login {
                    source: LoginError::CaptchaRequired,
                });
            };

            info!("TradingView requested captcha: dispatching single solve task to 2Captcha");
            let solved = solver.solve().await?;

            info!("Captcha solved successfully; submitting retry signin with token");
            let retry_response = client
                .post(&signin_url)
                .cookie_provider(std::sync::Arc::clone(&cookie_jar))
                .header(wreq::header::ORIGIN, base_url)
                .header(wreq::header::REFERER, format!("{base_url}/"))
                .form(&[
                    ("username", username),
                    ("password", password),
                    ("remember", "true"),
                    ("g-recaptcha-response-v2", solved.token.as_str()),
                ])
                .send()
                .await?;

            let retry_status = retry_response.status();
            if retry_status == wreq::StatusCode::TOO_MANY_REQUESTS {
                return Err(Error::RateLimited(
                    "HTTP 429 Too Many Requests during captcha retry".into(),
                ));
            }

            let retry_bytes = retry_response.bytes().await?;
            let retry_body: Value = match serde_json::from_slice(&retry_bytes) {
                Ok(val) => val,
                Err(_) => {
                    if !retry_status.is_success() {
                        return Err(Error::Request(
                            format!("HTTP signin failed with status {retry_status}").into(),
                        ));
                    }
                    return Err(Error::JsonParse(
                        "failed to parse signin JSON response after captcha".into(),
                    ));
                }
            };

            if is_rate_limited(retry_status, Some(&retry_body)) {
                return Err(Error::RateLimited(
                    "Rate limit exceeded during signin after captcha".into(),
                ));
            }

            if retry_status.is_server_error() {
                return Err(Error::Request(
                    format!("HTTP signin failed with status {retry_status}").into(),
                ));
            }

            if is_challenge_eligible_status(retry_status) && is_recaptcha_required(&retry_body) {
                match solver.report_incorrect(solved.task_id).await {
                    Ok(()) => {
                        info!(
                            "incorrect CAPTCHA solution reported; refund subject to provider review"
                        );
                        return Err(Error::Login {
                            source: LoginError::CaptchaRequired,
                        });
                    }
                    Err(report_err) => {
                        warn!("CAPTCHA rejected and incorrect-solution reporting failed");
                        return Err(report_err);
                    }
                }
            } else if is_recaptcha_required(&retry_body) {
                return Err(Error::Login {
                    source: LoginError::CaptchaRequired,
                });
            }

            (retry_body, retry_status)
        } else {
            (body, status)
        };

        if status.is_server_error() {
            return Err(Error::Request(
                format!("HTTP signin failed with status {status}").into(),
            ));
        }

        let err_str = body.get("error").and_then(|v| v.as_str()).unwrap_or("");
        let code_str = body.get("code").and_then(|v| v.as_str()).unwrap_or("");

        if err_str == "2FA_required" || code_str == "2FA_required" {
            let Some(secret) = totp_secret else {
                warn!("2FA is enabled for this account, but no TOTP secret was provided");
                return Err(Error::Login {
                    source: LoginError::OTPSecretNotFound,
                });
            };

            let trimmed_secret = secret.trim();
            if trimmed_secret.is_empty() {
                return Err(Error::Login {
                    source: LoginError::OTPSecretNotFound,
                });
            }

            let mfa_response =
                Self::handle_mfa(client, base_url, trimmed_secret, &cookie_jar).await?;

            let mfa_status = mfa_response.status();
            if mfa_status == wreq::StatusCode::TOO_MANY_REQUESTS {
                return Err(Error::RateLimited(
                    "HTTP 429 Too Many Requests during MFA".into(),
                ));
            }

            let mfa_bytes = mfa_response.bytes().await?;
            let mfa_body: Value = match serde_json::from_slice(&mfa_bytes) {
                Ok(val) => val,
                Err(_) => {
                    if !mfa_status.is_success() {
                        return Err(Error::Request(
                            format!("HTTP MFA request failed with status {mfa_status}").into(),
                        ));
                    }
                    return Err(Error::JsonParse("failed to parse MFA JSON response".into()));
                }
            };

            if is_rate_limited(mfa_status, Some(&mfa_body)) {
                return Err(Error::RateLimited("Rate limit exceeded during MFA".into()));
            }

            if is_recaptcha_required(&mfa_body) {
                warn!("TradingView MFA required captcha");
                return Err(Error::Login {
                    source: LoginError::CaptchaRequired,
                });
            }

            if mfa_status.is_server_error() {
                return Err(Error::Request(
                    format!("HTTP MFA request failed with status {mfa_status}").into(),
                ));
            }

            let mfa_err = mfa_body.get("error").and_then(|v| v.as_str()).unwrap_or("");
            let is_credential_rejection_status = mfa_status == wreq::StatusCode::BAD_REQUEST
                || mfa_status == wreq::StatusCode::UNAUTHORIZED
                || mfa_status == wreq::StatusCode::FORBIDDEN;

            if !mfa_err.is_empty() && (mfa_status.is_success() || is_credential_rejection_status) {
                warn!("TradingView MFA verification failed");
                return Err(Error::Login {
                    source: LoginError::InvalidOTPSecret,
                });
            }

            if !mfa_status.is_success() {
                return Err(Error::Request(
                    format!("HTTP MFA request failed with status {mfa_status}").into(),
                ));
            }

            let session_target_uri = format!("{base_url}/quote_token/");
            let final_session = get_cookie_value(&cookie_jar, &session_target_uri, "sessionid");
            let final_signature =
                get_cookie_value(&cookie_jar, &session_target_uri, "sessionid_sign");
            let final_device_token = get_cookie_value(&cookie_jar, &session_target_uri, "device_t");

            if final_session.trim().is_empty() || final_signature.trim().is_empty() {
                warn!("unable to login, session cookies missing in MFA response");
                return Err(Error::Login {
                    source: LoginError::SessionNotFound,
                });
            }

            let login_resp: LoginUserResponse = serde_json::from_value(mfa_body)
                .map_err(|_| Error::JsonParse("failed to parse MFA user response".into()))?;

            info!("2FA authentication completed");
            info!("User is logged in successfully");

            Ok(UserCookies {
                session: final_session,
                session_signature: final_signature,
                device_token: final_device_token,
                ..login_resp.user
            })
        } else if err_str.is_empty() {
            if !status.is_success() {
                return Err(Error::Request(
                    format!("HTTP signin failed with status {status}").into(),
                ));
            }

            let session_target_uri = format!("{base_url}/quote_token/");
            let final_session = get_cookie_value(&cookie_jar, &session_target_uri, "sessionid");
            let final_signature =
                get_cookie_value(&cookie_jar, &session_target_uri, "sessionid_sign");
            let final_device_token = get_cookie_value(&cookie_jar, &session_target_uri, "device_t");

            if final_session.trim().is_empty() || final_signature.trim().is_empty() {
                warn!("unable to login, session cookies missing in signin response");
                return Err(Error::Login {
                    source: LoginError::SessionNotFound,
                });
            }

            warn!("2FA is not enabled for this account");
            info!("User is logged in successfully");
            let login_resp: LoginUserResponse = serde_json::from_value(body)
                .map_err(|_| Error::JsonParse("failed to parse login user response".into()))?;

            Ok(UserCookies {
                session: final_session,
                session_signature: final_signature,
                device_token: final_device_token,
                ..login_resp.user
            })
        } else {
            let is_credential_rejection_status = status == wreq::StatusCode::BAD_REQUEST
                || status == wreq::StatusCode::UNAUTHORIZED
                || status == wreq::StatusCode::FORBIDDEN;

            if status.is_success() || is_credential_rejection_status {
                warn!("TradingView signin failed with server error");
                Err(Error::Login {
                    source: LoginError::InvalidCredentials,
                })
            } else {
                Err(Error::Request(
                    format!("HTTP signin failed with status {status}").into(),
                ))
            }
        }
    }

    async fn handle_mfa(
        client: &wreq::Client,
        base_url: &str,
        totp_secret: &str,
        cookie_jar: &std::sync::Arc<wreq::cookie::Jar>,
    ) -> Result<Response> {
        let trimmed = totp_secret.trim();
        if trimmed.is_empty() {
            return Err(Error::Login {
                source: LoginError::OTPSecretNotFound,
            });
        }

        let code = generate_totp_code(trimmed).map_err(|_| {
            warn!("invalid TOTP configuration");
            Error::Login {
                source: LoginError::InvalidOTPSecret,
            }
        })?;

        let mfa_url = format!("{base_url}/accounts/two-factor/signin/totp/");
        let response = client
            .post(&mfa_url)
            .cookie_provider(std::sync::Arc::clone(cookie_jar))
            .header(wreq::header::ORIGIN, base_url)
            .header(wreq::header::REFERER, format!("{base_url}/"))
            .form(&[("code", code.as_str())])
            .send()
            .await?;

        Ok(response)
    }
}

pub async fn fetch_tradingview_token(client: &UserCookies) -> Result<String> {
    fetch_tradingview_token_internal(user_http_client(), "https://www.tradingview.com", client)
        .await
}

async fn fetch_tradingview_token_internal(
    client_http: &wreq::Client,
    base_url: &str,
    user: &UserCookies,
) -> Result<String> {
    if user.session.trim().is_empty() || user.session_signature.trim().is_empty() {
        return Err(Error::Login {
            source: LoginError::SessionNotFound,
        });
    }

    let mut cookie_parts = vec![
        format!("sessionid={}", user.session.trim()),
        format!("sessionid_sign={}", user.session_signature.trim()),
    ];
    if !user.device_token.trim().is_empty() {
        cookie_parts.push(format!("device_t={}", user.device_token.trim()));
    }
    let cookie = cookie_parts.join("; ");

    let url = format!("{base_url}/quote_token/");
    let resp = client_http
        .get(&url)
        .header(COOKIE, &cookie)
        .header(wreq::header::ORIGIN, base_url)
        .header(wreq::header::REFERER, format!("{base_url}/"))
        .send()
        .await?;

    let status = resp.status();
    if status == wreq::StatusCode::TOO_MANY_REQUESTS {
        return Err(Error::RateLimited(
            "HTTP 429 Too Many Requests: /quote_token/".into(),
        ));
    }
    if status == wreq::StatusCode::FORBIDDEN || status == wreq::StatusCode::UNAUTHORIZED {
        return Err(Error::Login {
            source: LoginError::InvalidSession,
        });
    }
    if !status.is_success() {
        return Err(Error::Request(
            format!("HTTP request failed with status {status}: /quote_token/").into(),
        ));
    }

    let bytes = resp.bytes().await?;
    let token: String = serde_json::from_slice(&bytes)
        .map_err(|_| Error::JsonParse("failed to parse quote token JSON string".into()))?;

    if token.trim().is_empty() {
        return Err(Error::NoChartTokenFound);
    }
    Ok(token)
}
pub(crate) fn is_recaptcha_required(body: &Value) -> bool {
    let code_match = body
        .get("code")
        .and_then(|v| v.as_str())
        .is_some_and(|c| c == "recaptcha_required");
    let error_match = body
        .get("error")
        .and_then(|v| v.as_str())
        .is_some_and(|e| e == "recaptcha_required");
    code_match || error_match
}

pub(crate) fn create_totp(secret_or_uri: &str) -> std::result::Result<Totp, TotpError> {
    let trimmed = secret_or_uri.trim();

    if trimmed.starts_with("otpauth://") {
        match Totp::from_url(trimmed) {
            Ok(totp) => Ok(totp),
            Err(TotpError::SecretTooShort { .. }) => {
                let totp = Totp::from_url_unchecked(trimmed)?;
                if totp.step() == 0 {
                    return Err(TotpError::InvalidStepZero);
                }
                Ok(totp)
            }
            Err(err) => Err(err),
        }
    } else {
        let normalized_secret = if trimmed.bytes().any(|b| b.is_ascii_whitespace()) {
            std::borrow::Cow::Owned(
                trimmed
                    .chars()
                    .filter(|c| !c.is_ascii_whitespace())
                    .collect::<String>(),
            )
        } else {
            std::borrow::Cow::Borrowed(trimmed)
        };
        let sec = Secret::try_from_base32(normalized_secret.as_ref())
            .map_err(|_| TotpError::InvalidSecret)?;
        let builder = Builder::new().with_secret(sec);
        match builder.build() {
            Ok(totp) => Ok(totp),
            Err(TotpError::SecretTooShort { .. }) => {
                let sec = Secret::try_from_base32(normalized_secret.as_ref())
                    .map_err(|_| TotpError::InvalidSecret)?;
                Ok(Builder::new().with_secret(sec).build_noncompliant())
            }
            Err(err) => Err(err),
        }
    }
}

pub(crate) fn generate_totp_code(secret_or_uri: &str) -> std::result::Result<String, TotpError> {
    let totp = create_totp(secret_or_uri)?;
    Ok(totp.generate_current().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    #[test]
    fn test_rfc6238_sha1_vectors() {
        // RFC 6238 Appendix B test vector: secret "12345678901234567890" -> Base32 "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ"
        let secret = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";
        let totp = create_totp(secret).expect("valid RFC 6238 secret");

        assert_eq!(totp.generate(59).to_string(), "287082");
        assert_eq!(totp.generate(1111111109).to_string(), "081804");
        assert_eq!(totp.generate(1234567890).to_string(), "005924");
    }

    #[test]
    fn test_legacy_16char_base32_and_uri_equivalence() {
        let legacy_b32 = "JBSWY3DPEHPK3PXP";
        let totp_b32 = create_totp(legacy_b32).expect("legacy 16-char base32 secret accepted");

        let uri =
            format!("otpauth://totp/TradingView:alice?secret={legacy_b32}&issuer=TradingView");
        let totp_uri = create_totp(&uri).expect("otpauth URI with 16-char secret accepted");

        let timestamp = 1234567890;
        assert_eq!(
            totp_b32.generate(timestamp).to_string(),
            totp_uri.generate(timestamp).to_string()
        );
        assert_eq!(totp_b32.generate(timestamp).to_string().len(), 6);
    }

    #[test]
    fn test_grouped_base32_secrets_equivalence() {
        // RFC 6238 Appendix B 32-char Base32 secret with spaces, tabs, and newlines
        let raw_rfc = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";
        let grouped_spaces = "GEZD GNBV GY3T QOJQ GEZD GNBV GY3T QOJQ";
        let grouped_mixed_ws = "  GEZDGNBV\tGY3TQOJQ\nGEZDGNBV\r\nGY3TQOJQ  ";

        let totp_clean = create_totp(raw_rfc).expect("clean RFC base32 secret accepted");
        let totp_spaces =
            create_totp(grouped_spaces).expect("space-grouped base32 secret accepted");
        let totp_mixed =
            create_totp(grouped_mixed_ws).expect("mixed-whitespace base32 secret accepted");

        let timestamp = 1234567890;
        let expected_code = "005924";
        assert_eq!(totp_clean.generate(timestamp).to_string(), expected_code);
        assert_eq!(totp_spaces.generate(timestamp).to_string(), expected_code);
        assert_eq!(totp_mixed.generate(timestamp).to_string(), expected_code);

        // Grouped legacy short secret (16-char Base32)
        let legacy_grouped = "JBSW Y3DP EHPK 3PXP";
        let totp_legacy_clean = create_totp("JBSWY3DPEHPK3PXP").expect("legacy clean accepted");
        let totp_legacy_grouped =
            create_totp(legacy_grouped).expect("legacy grouped base32 accepted");
        assert_eq!(
            totp_legacy_clean.generate(timestamp).to_string(),
            totp_legacy_grouped.generate(timestamp).to_string()
        );

        // Punctuation and invalid non-whitespace separators are strictly rejected
        assert!(create_totp("GEZD-GNBV-GY3T-QOJQ-GEZD-GNBV-GY3T-QOJQ").is_err());
        assert!(create_totp("GEZD.GNBV.GY3T.QOJQ").is_err());
        assert!(create_totp("GEZD,GNBV").is_err());
        assert!(create_totp("JBSW-Y3DP-EHPK-3PXP").is_err());
    }

    #[test]
    fn test_invalid_totp_secrets() {
        // Invalid base32 characters (1, 8, 9, 0 are not standard base32)
        assert!(create_totp("INVALID890!@#").is_err());
        assert!(generate_totp_code("INVALID890!@#").is_err());

        // Invalid scheme
        assert!(create_totp("https://example.com/totp").is_err());

        // Zero step
        let zero_step_uri = "otpauth://totp/TradingView:alice?secret=JBSWY3DPEHPK3PXP&period=0";
        assert!(create_totp(zero_step_uri).is_err());
    }
    #[test]
    fn test_recaptcha_required_parsing() {
        let recorded_code_shape = serde_json::json!({
            "code": "recaptcha_required",
            "error": "Please confirm you are not a robot"
        });
        assert!(is_recaptcha_required(&recorded_code_shape));

        let recorded_error_shape = serde_json::json!({
            "error": "recaptcha_required"
        });
        assert!(is_recaptcha_required(&recorded_error_shape));

        let normal_error_shape = serde_json::json!({
            "error": "invalid_credentials"
        });
        assert!(!is_recaptcha_required(&normal_error_shape));

        let success_shape = serde_json::json!({
            "error": ""
        });
        assert!(!is_recaptcha_required(&success_shape));
    }

    use std::sync::Arc;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::sync::oneshot;

    struct MockServerRequest {
        #[allow(dead_code)]
        method: String,
        path: String,
        headers: Vec<(String, String)>,
        body: Vec<u8>,
    }

    impl MockServerRequest {
        fn header(&self, name: &str) -> Option<&str> {
            self.headers
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(name))
                .map(|(_, v)| v.as_str())
        }
    }

    struct MockServerResponse {
        status: u16,
        headers: Vec<(&'static str, String)>,
        body: Vec<u8>,
        override_content_length: Option<usize>,
    }

    impl MockServerResponse {
        fn json(status: u16, json: &str) -> Self {
            Self {
                status,
                headers: vec![("Content-Type", "application/json".to_string())],
                body: json.as_bytes().to_vec(),
                override_content_length: None,
            }
        }

        fn text(status: u16, text: &str, content_type: &'static str) -> Self {
            Self {
                status,
                headers: vec![("Content-Type", content_type.to_string())],
                body: text.as_bytes().to_vec(),
                override_content_length: None,
            }
        }

        fn with_cookie(self, cookie_val: &str) -> Self {
            self.with_header("Set-Cookie", cookie_val)
        }

        fn with_header(mut self, name: &'static str, value: impl Into<String>) -> Self {
            self.headers.push((name, value.into()));
            self
        }

        fn with_override_content_length(mut self, len: usize) -> Self {
            self.override_content_length = Some(len);
            self
        }
    }

    async fn spawn_mock_server<F>(handler: F) -> (String, oneshot::Sender<()>)
    where
        F: Fn(MockServerRequest) -> MockServerResponse + Send + Sync + 'static,
    {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind listener");
        let local_addr = listener.local_addr().expect("local addr");
        let base_url = format!("http://127.0.0.1:{}", local_addr.port());
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel();
        let handler = Arc::new(handler);

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = &mut shutdown_rx => {
                        break;
                    }
                    res = listener.accept() => {
                        let Ok((mut socket, _)) = res else { break };
                        let handler = Arc::clone(&handler);
                        tokio::spawn(async move {
                            let mut buf = vec![0u8; 8192];
                            let mut n = 0;
                            while n < buf.len() {
                                let read_bytes = socket.read(&mut buf[n..]).await.unwrap_or(0);
                                if read_bytes == 0 {
                                    break;
                                }
                                n += read_bytes;
                                if buf[..n].windows(4).any(|w| w == b"\r\n\r\n") {
                                    break;
                                }
                            }
                            if n == 0 {
                                return;
                            }
                            let header_end = buf[..n]
                                .windows(4)
                                .position(|w| w == b"\r\n\r\n")
                                .unwrap_or(n);
                            let header_str = String::from_utf8_lossy(&buf[..header_end]);
                            let mut lines = header_str.lines();
                            let first_line = lines.next().unwrap_or("");
                            let mut parts = first_line.split_whitespace();
                            let method = parts.next().unwrap_or("GET").to_string();
                            let path = parts.next().unwrap_or("/").to_string();

                            let mut headers = Vec::new();
                            for line in lines {
                                if let Some((k, v)) = line.split_once(':') {
                                    headers.push((k.trim().to_string(), v.trim().to_string()));
                                }
                            }

                            let content_length = headers
                                .iter()
                                .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
                                .and_then(|(_, v)| v.parse::<usize>().ok())
                                .unwrap_or(0);

                            let mut body = buf[header_end + 4..n].to_vec();
                            while body.len() < content_length {
                                let mut extra = vec![0u8; content_length - body.len()];
                                let read_bytes = socket.read(&mut extra).await.unwrap_or(0);
                                if read_bytes == 0 {
                                    break;
                                }
                                body.extend_from_slice(&extra[..read_bytes]);
                            }

                            let req = MockServerRequest {
                                method,
                                path,
                                headers,
                                body,
                            };
                            let resp = handler(req);

                            let reason = match resp.status {
                                200 => "OK",
                                400 => "Bad Request",
                                401 => "Unauthorized",
                                403 => "Forbidden",
                                429 => "Too Many Requests",
                                500 => "Internal Server Error",
                                _ => "Unknown",
                            };

                            let content_len = resp.override_content_length.unwrap_or(resp.body.len());
                            let mut response_bytes = format!(
                                "HTTP/1.1 {} {}\r\nContent-Length: {}\r\nConnection: close\r\n",
                                resp.status,
                                reason,
                                content_len
                            );
                            for (name, val) in resp.headers {
                                response_bytes.push_str(&format!("{name}: {val}\r\n"));
                            }
                            response_bytes.push_str("\r\n");

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

    fn test_client() -> wreq::Client {
        wreq::Client::builder().build().expect("build test client")
    }

    #[tokio::test]
    async fn test_mock_login_password_success() {
        let (base_url, _shutdown) = spawn_mock_server(|req| {
            assert_eq!(req.path, "/accounts/signin/");
            assert_eq!(req.method, "POST");
            let body_str = String::from_utf8_lossy(&req.body);
            assert!(body_str.contains("username=alice"));
            assert!(body_str.contains("password=secret"));

            MockServerResponse::json(
                200,
                r#"{"user":{"id":42,"username":"alice","private_channel":"p1","auth_token":"a1","session_hash":"h1"},"error":""}"#,
            )
            .with_cookie("sessionid=sess_123; Path=/")
            .with_cookie("sessionid_sign=sig_123; Path=/")
            .with_cookie("device_t=dev_123; Path=/")
        })
        .await;

        let client = test_client();
        let mut user = UserCookies::new();
        let logged_in = user
            .login_internal(&client, &base_url, "alice", "secret", None)
            .await
            .expect("login should succeed");

        assert_eq!(logged_in.id, 42);
        assert_eq!(logged_in.username, "alice");
        assert_eq!(logged_in.session, "sess_123");
        assert_eq!(logged_in.session_signature, "sig_123");
        assert_eq!(logged_in.device_token, "dev_123");
        assert_eq!(logged_in.session_hash, "h1");
    }

    #[tokio::test]
    async fn test_mock_login_cookieless_2fa_challenge_and_rotation() {
        let (base_url, _shutdown) = spawn_mock_server(|req| {
            if req.path == "/accounts/signin/" {
                MockServerResponse::json(200, r#"{"error":"2FA_required"}"#)
            } else if req.path == "/accounts/two-factor/signin/totp/" {
                assert_eq!(req.method, "POST");
                let body_str = String::from_utf8_lossy(&req.body);
                assert!(body_str.contains("code="));
                MockServerResponse::json(
                    200,
                    r#"{"user":{"id":99,"username":"bob","private_channel":"p2","auth_token":"a2","session_hash":"h2"},"error":""}"#,
                )
                .with_cookie("sessionid=mfa_sess; Path=/")
                .with_cookie("sessionid_sign=mfa_sig; Path=/")
                .with_cookie("device_t=mfa_dev; Path=/")
            } else {
                panic!("unexpected path: {}", req.path);
            }
        })
        .await;

        let client = test_client();
        let mut user = UserCookies::new();
        let totp_secret = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";
        let logged_in = user
            .login_internal(&client, &base_url, "bob", "secret", Some(totp_secret))
            .await
            .expect("2FA login should succeed");

        assert_eq!(logged_in.id, 99);
        assert_eq!(logged_in.username, "bob");
        assert_eq!(logged_in.session, "mfa_sess");
        assert_eq!(logged_in.session_signature, "mfa_sig");
        assert_eq!(logged_in.device_token, "mfa_dev");
    }

    #[tokio::test]
    async fn test_mock_login_2fa_device_forwarding_and_cookie_merge() {
        let (base_url, _shutdown) = spawn_mock_server(|req| {
            if req.path == "/accounts/signin/" {
                MockServerResponse::json(200, r#"{"error":"2FA_required"}"#)
                    .with_cookie("sessionid=old_sess; Path=/")
                    .with_cookie("sessionid_sign=old_sig; Path=/")
                    .with_cookie("device_t=initial_device_token; Path=/")
            } else if req.path == "/accounts/two-factor/signin/totp/" {
                let cookie_hdr = req.header("cookie").expect("cookie header in MFA request");
                assert!(cookie_hdr.contains("device_t=initial_device_token"));
                assert!(cookie_hdr.contains("sessionid=old_sess"));
                assert!(cookie_hdr.contains("sessionid_sign=old_sig"));

                MockServerResponse::json(
                    200,
                    r#"{"user":{"id":10,"username":"carol","private_channel":"p3","auth_token":"a3","session_hash":"h3"},"error":""}"#,
                )
                .with_cookie("sessionid=rotated_sess; Path=/")
                .with_cookie("sessionid_sign=rotated_sig; Path=/")
            } else {
                panic!("unexpected path: {}", req.path);
            }
        })
        .await;

        let client = test_client();
        let mut user = UserCookies::new();
        let totp_secret = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";
        let logged_in = user
            .login_internal(&client, &base_url, "carol", "secret", Some(totp_secret))
            .await
            .expect("login with device forwarding should succeed");

        assert_eq!(logged_in.session, "rotated_sess");
        assert_eq!(logged_in.session_signature, "rotated_sig");
        assert_eq!(logged_in.device_token, "initial_device_token");
    }

    #[tokio::test]
    async fn test_mock_login_missing_totp_when_2fa_required() {
        let (base_url, _shutdown) =
            spawn_mock_server(|_| MockServerResponse::json(200, r#"{"error":"2FA_required"}"#))
                .await;

        let client = test_client();
        let mut user = UserCookies::new();
        let err = user
            .login_internal(&client, &base_url, "alice", "secret", None)
            .await
            .unwrap_err();
        assert!(
            matches!(
                err,
                Error::Login {
                    source: LoginError::OTPSecretNotFound
                }
            ),
            "expected OTPSecretNotFound on absent TOTP, got: {err:?}"
        );

        let err_empty = user
            .login_internal(&client, &base_url, "alice", "secret", Some("   "))
            .await
            .unwrap_err();
        assert!(
            matches!(
                err_empty,
                Error::Login {
                    source: LoginError::OTPSecretNotFound
                }
            ),
            "expected OTPSecretNotFound on empty TOTP, got: {err_empty:?}"
        );
    }

    #[tokio::test]
    async fn test_mock_login_invalid_totp_secret() {
        let (base_url, _shutdown) =
            spawn_mock_server(|_| MockServerResponse::json(200, r#"{"error":"2FA_required"}"#))
                .await;

        let client = test_client();
        let mut user = UserCookies::new();
        let err = user
            .login_internal(
                &client,
                &base_url,
                "alice",
                "secret",
                Some("INVALID!!BASE32"),
            )
            .await
            .unwrap_err();
        assert!(
            matches!(
                err,
                Error::Login {
                    source: LoginError::InvalidOTPSecret
                }
            ),
            "expected InvalidOTPSecret on malformed base32, got: {err:?}"
        );
    }

    #[tokio::test]
    async fn test_mock_login_totp_rejected_by_server() {
        let (base_url, _shutdown) = spawn_mock_server(|req| {
            if req.path == "/accounts/signin/" {
                MockServerResponse::json(200, r#"{"error":"2FA_required"}"#)
            } else {
                MockServerResponse::json(200, r#"{"error":"wrong_code"}"#)
            }
        })
        .await;

        let client = test_client();
        let mut user = UserCookies::new();
        let totp_secret = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";
        let err = user
            .login_internal(&client, &base_url, "alice", "secret", Some(totp_secret))
            .await
            .unwrap_err();
        assert!(
            matches!(
                err,
                Error::Login {
                    source: LoginError::InvalidOTPSecret
                }
            ),
            "expected InvalidOTPSecret when TOTP rejected by server, got: {err:?}"
        );
    }

    #[tokio::test]
    async fn test_mock_login_missing_final_cookies() {
        let (base_url, _shutdown) = spawn_mock_server(|_| {
            MockServerResponse::json(
                200,
                r#"{"user":{"id":1,"username":"a","private_channel":"p","auth_token":"t","session_hash":"h"},"error":""}"#,
            )
        })
        .await;

        let client = test_client();
        let mut user = UserCookies::new();
        let err = user
            .login_internal(&client, &base_url, "alice", "secret", None)
            .await
            .unwrap_err();
        assert!(
            matches!(
                err,
                Error::Login {
                    source: LoginError::SessionNotFound
                }
            ),
            "expected SessionNotFound when cookies missing in password response, got: {err:?}"
        );

        let (base_url2, _shutdown2) = spawn_mock_server(|req| {
            if req.path == "/accounts/signin/" {
                MockServerResponse::json(200, r#"{"error":"2FA_required"}"#)
            } else {
                MockServerResponse::json(
                    200,
                    r#"{"user":{"id":1,"username":"a","private_channel":"p","auth_token":"t","session_hash":"h"},"error":""}"#,
                )
            }
        })
        .await;

        let err2 = user
            .login_internal(
                &client,
                &base_url2,
                "alice",
                "secret",
                Some("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ"),
            )
            .await
            .unwrap_err();
        assert!(
            matches!(
                err2,
                Error::Login {
                    source: LoginError::SessionNotFound
                }
            ),
            "expected SessionNotFound when cookies missing in MFA response, got: {err2:?}"
        );
    }

    #[tokio::test]
    async fn test_mock_login_captcha_both_phases() {
        let (base_url, _shutdown) = spawn_mock_server(|_| {
            MockServerResponse::json(
                200,
                r#"{"code":"recaptcha_required","error":"Captcha needed"}"#,
            )
        })
        .await;

        let client = test_client();
        let mut user = UserCookies::new();
        let err = user
            .login_internal(&client, &base_url, "alice", "secret", None)
            .await
            .unwrap_err();
        assert!(
            matches!(
                err,
                Error::Login {
                    source: LoginError::CaptchaRequired
                }
            ),
            "expected CaptchaRequired during signin, got: {err:?}"
        );

        let (base_url2, _shutdown2) = spawn_mock_server(|req| {
            if req.path == "/accounts/signin/" {
                MockServerResponse::json(200, r#"{"error":"2FA_required"}"#)
            } else {
                MockServerResponse::json(200, r#"{"code":"recaptcha_required"}"#)
            }
        })
        .await;

        let err2 = user
            .login_internal(
                &client,
                &base_url2,
                "alice",
                "secret",
                Some("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ"),
            )
            .await
            .unwrap_err();
        assert!(
            matches!(
                err2,
                Error::Login {
                    source: LoginError::CaptchaRequired
                }
            ),
            "expected CaptchaRequired during MFA, got: {err2:?}"
        );
    }

    #[tokio::test]
    async fn test_mock_login_429_both_phases() {
        let (base_url, _shutdown) =
            spawn_mock_server(|_| MockServerResponse::text(429, "Too Many Requests", "text/plain"))
                .await;

        let client = test_client();
        let mut user = UserCookies::new();
        let err = user
            .login_internal(&client, &base_url, "alice", "secret", None)
            .await
            .unwrap_err();
        assert!(
            matches!(err, Error::RateLimited(_)),
            "expected RateLimited during signin, got: {err:?}"
        );

        let (base_url2, _shutdown2) = spawn_mock_server(|req| {
            if req.path == "/accounts/signin/" {
                MockServerResponse::json(200, r#"{"error":"2FA_required"}"#)
            } else {
                MockServerResponse::text(429, "Too Many Requests", "text/plain")
            }
        })
        .await;

        let err2 = user
            .login_internal(
                &client,
                &base_url2,
                "alice",
                "secret",
                Some("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ"),
            )
            .await
            .unwrap_err();
        assert!(
            matches!(err2, Error::RateLimited(_)),
            "expected RateLimited during MFA, got: {err2:?}"
        );
    }

    #[tokio::test]
    async fn test_mock_login_html_status_and_malformed_payload() {
        let (base_url, _shutdown) = spawn_mock_server(|_| {
            MockServerResponse::text(500, "Internal Server Error", "text/plain")
        })
        .await;

        let client = test_client();
        let mut user = UserCookies::new();
        let err = user
            .login_internal(&client, &base_url, "alice", "secret", None)
            .await
            .unwrap_err();
        assert!(
            matches!(err, Error::Request(_)),
            "expected Error::Request on HTTP 500, got: {err:?}"
        );

        let (base_url2, _shutdown2) = spawn_mock_server(|_| {
            MockServerResponse::text(
                200,
                "<html><body>Cloudflare Block</body></html>",
                "text/html",
            )
        })
        .await;

        let err2 = user
            .login_internal(&client, &base_url2, "alice", "secret", None)
            .await
            .unwrap_err();
        assert!(
            matches!(err2, Error::JsonParse(_)),
            "expected Error::JsonParse on 200 HTML, got: {err2:?}"
        );

        let (base_url3, _shutdown3) = spawn_mock_server(|req| {
            if req.path == "/accounts/signin/" {
                MockServerResponse::json(200, r#"{"error":"2FA_required"}"#)
            } else {
                MockServerResponse::text(500, "Internal Server Error", "text/plain")
            }
        })
        .await;

        let err3 = user
            .login_internal(
                &client,
                &base_url3,
                "alice",
                "secret",
                Some("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ"),
            )
            .await
            .unwrap_err();
        assert!(
            matches!(err3, Error::Request(_)),
            "expected Error::Request on MFA HTTP 500, got: {err3:?}"
        );

        let (base_url4, _shutdown4) = spawn_mock_server(|req| {
            if req.path == "/accounts/signin/" {
                MockServerResponse::json(200, r#"{"error":"2FA_required"}"#)
            } else {
                MockServerResponse::text(
                    200,
                    "<html><body>HTML instead of JSON</body></html>",
                    "text/html",
                )
            }
        })
        .await;

        let err4 = user
            .login_internal(
                &client,
                &base_url4,
                "alice",
                "secret",
                Some("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ"),
            )
            .await
            .unwrap_err();
        assert!(
            matches!(err4, Error::JsonParse(_)),
            "expected Error::JsonParse on MFA 200 HTML, got: {err4:?}"
        );
    }

    #[tokio::test]
    async fn test_mock_quote_token_success_and_failures() {
        let client = test_client();
        let user = UserCookies {
            session: "sess_ok".to_string(),
            session_signature: "sig_ok".to_string(),
            device_token: "dev_ok".to_string(),
            ..Default::default()
        };

        // 1. Success
        let (base_url, _s) = spawn_mock_server(|req| {
            assert_eq!(req.path, "/quote_token/");
            let cookie = req.header("cookie").unwrap();
            assert!(cookie.contains("sessionid=sess_ok"));
            assert!(cookie.contains("sessionid_sign=sig_ok"));
            assert!(cookie.contains("device_t=dev_ok"));
            MockServerResponse::json(200, r#""valid_token_xyz_123""#)
        })
        .await;

        let token = fetch_tradingview_token_internal(&client, &base_url, &user)
            .await
            .expect("token fetch should succeed");
        assert_eq!(token, "valid_token_xyz_123");

        // 2. HTTP 403 Forbidden -> InvalidSession
        let (base_url_403, _s) =
            spawn_mock_server(|_| MockServerResponse::text(403, "Forbidden", "text/plain")).await;
        let err_403 = fetch_tradingview_token_internal(&client, &base_url_403, &user)
            .await
            .unwrap_err();
        assert!(
            matches!(
                err_403,
                Error::Login {
                    source: LoginError::InvalidSession
                }
            ),
            "expected InvalidSession on HTTP 403, got: {err_403:?}"
        );

        // 3. HTTP 401 Unauthorized -> InvalidSession
        let (base_url_401, _s) =
            spawn_mock_server(|_| MockServerResponse::text(401, "Unauthorized", "text/plain"))
                .await;
        let err_401 = fetch_tradingview_token_internal(&client, &base_url_401, &user)
            .await
            .unwrap_err();
        assert!(
            matches!(
                err_401,
                Error::Login {
                    source: LoginError::InvalidSession
                }
            ),
            "expected InvalidSession on HTTP 401, got: {err_401:?}"
        );

        // 4. HTTP 429 Too Many Requests -> RateLimited
        let (base_url_429, _s) =
            spawn_mock_server(|_| MockServerResponse::text(429, "Too Many Requests", "text/plain"))
                .await;
        let err_429 = fetch_tradingview_token_internal(&client, &base_url_429, &user)
            .await
            .unwrap_err();
        assert!(
            matches!(err_429, Error::RateLimited(_)),
            "expected RateLimited on HTTP 429, got: {err_429:?}"
        );

        // 5. HTTP 500 -> Request
        let (base_url_500, _s) =
            spawn_mock_server(|_| MockServerResponse::text(500, "Server Error", "text/plain"))
                .await;
        let err_500 = fetch_tradingview_token_internal(&client, &base_url_500, &user)
            .await
            .unwrap_err();
        assert!(
            matches!(err_500, Error::Request(_)),
            "expected Error::Request on HTTP 500, got: {err_500:?}"
        );

        // 6. HTML response on 200 -> JsonParse
        let (base_url_html, _s) = spawn_mock_server(|_| {
            MockServerResponse::text(
                200,
                "<html><body>HTML Token Error</body></html>",
                "text/html",
            )
        })
        .await;
        let err_html = fetch_tradingview_token_internal(&client, &base_url_html, &user)
            .await
            .unwrap_err();
        assert!(
            matches!(err_html, Error::JsonParse(_)),
            "expected JsonParse on HTML response, got: {err_html:?}"
        );

        // 7. Object response on 200 -> JsonParse (strict JSON string parsing)
        let (base_url_obj, _s) = spawn_mock_server(|_| {
            MockServerResponse::json(200, r#"{"token":"should_be_strict_string"}"#)
        })
        .await;
        let err_obj = fetch_tradingview_token_internal(&client, &base_url_obj, &user)
            .await
            .unwrap_err();
        assert!(
            matches!(err_obj, Error::JsonParse(_)),
            "expected JsonParse on JSON object response, got: {err_obj:?}"
        );

        // 8. Empty string JSON -> NoChartTokenFound
        let (base_url_empty, _s) =
            spawn_mock_server(|_| MockServerResponse::json(200, r#""""#)).await;
        let err_empty = fetch_tradingview_token_internal(&client, &base_url_empty, &user)
            .await
            .unwrap_err();
        assert!(
            matches!(err_empty, Error::NoChartTokenFound),
            "expected NoChartTokenFound on empty JSON string, got: {err_empty:?}"
        );
    }

    #[tokio::test]
    async fn test_mock_login_2fa_cleared_cookie_does_not_resurrect_initial() {
        let (base_url, _shutdown) = spawn_mock_server(|req| {
            if req.path == "/accounts/signin/" {
                MockServerResponse::json(200, r#"{"error":"2FA_required"}"#)
                    .with_cookie("sessionid=old_sess; Path=/")
                    .with_cookie("sessionid_sign=old_sig; Path=/")
            } else if req.path == "/accounts/two-factor/signin/totp/" {
                MockServerResponse::json(
                    200,
                    r#"{"user":{"id":10,"username":"carol","private_channel":"p3","auth_token":"a3","session_hash":"h3"},"error":""}"#,
                )
                .with_cookie("sessionid=; Path=/")
                .with_cookie("sessionid_sign=rotated_sig; Path=/")
            } else {
                panic!("unexpected path: {}", req.path);
            }
        })
        .await;

        let client = test_client();
        let mut user = UserCookies::new();
        let totp_secret = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";
        let err = user
            .login_internal(&client, &base_url, "carol", "secret", Some(totp_secret))
            .await
            .unwrap_err();

        assert!(
            matches!(
                err,
                Error::Login {
                    source: LoginError::SessionNotFound
                }
            ),
            "explicitly cleared sessionid must not resurrect old session, got: {err:?}"
        );
    }

    #[tokio::test]
    async fn test_mock_login_http_500_with_json_error_regression_signin_and_mfa() {
        let (base_url, _shutdown) = spawn_mock_server(|_| {
            MockServerResponse::json(500, r#"{"error":"internal_server_error"}"#)
        })
        .await;

        let client = test_client();
        let mut user = UserCookies::new();
        let err = user
            .login_internal(&client, &base_url, "alice", "secret", None)
            .await
            .unwrap_err();
        assert!(
            matches!(err, Error::Request(_)),
            "signin HTTP 500 with JSON error must be Error::Request, got: {err:?}"
        );

        let (base_url2, _shutdown2) = spawn_mock_server(|req| {
            if req.path == "/accounts/signin/" {
                MockServerResponse::json(200, r#"{"error":"2FA_required"}"#)
            } else {
                MockServerResponse::json(500, r#"{"error":"internal_server_error"}"#)
            }
        })
        .await;

        let err2 = user
            .login_internal(
                &client,
                &base_url2,
                "alice",
                "secret",
                Some("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ"),
            )
            .await
            .unwrap_err();
        assert!(
            matches!(err2, Error::Request(_)),
            "MFA HTTP 500 with JSON error must be Error::Request, got: {err2:?}"
        );
    }

    #[tokio::test]
    async fn test_mock_login_wrong_password_http200_json_error() {
        let (base_url, _shutdown) = spawn_mock_server(|_| {
            MockServerResponse::json(200, r#"{"error":"username or password is not correct"}"#)
        })
        .await;

        let client = test_client();
        let mut user = UserCookies::new();
        let err = user
            .login_internal(&client, &base_url, "alice", "wrong_pass", None)
            .await
            .unwrap_err();
        assert!(
            matches!(
                err,
                Error::Login {
                    source: LoginError::InvalidCredentials
                }
            ),
            "HTTP 200 wrong password must be InvalidCredentials, got: {err:?}"
        );
    }

    #[tokio::test]
    async fn test_mock_login_wrong_password_http401_json_error() {
        let (base_url, _shutdown) = spawn_mock_server(|_| {
            MockServerResponse::json(401, r#"{"error":"invalid_credentials"}"#)
        })
        .await;

        let client = test_client();
        let mut user = UserCookies::new();
        let err = user
            .login_internal(&client, &base_url, "alice", "wrong_pass", None)
            .await
            .unwrap_err();
        assert!(
            matches!(
                err,
                Error::Login {
                    source: LoginError::InvalidCredentials
                }
            ),
            "HTTP 401 wrong password must be InvalidCredentials, got: {err:?}"
        );
    }

    #[tokio::test]
    async fn test_mock_login_truncated_body_propagates_transport_error() {
        let (base_url, _shutdown) = spawn_mock_server(|_| {
            MockServerResponse::json(200, r#"{"user":{"id":1"#).with_override_content_length(5000)
        })
        .await;

        let client = test_client();
        let mut user = UserCookies::new();
        let err = user
            .login_internal(&client, &base_url, "alice", "secret", None)
            .await
            .unwrap_err();

        assert!(
            matches!(err, Error::Request(_)),
            "truncated body must propagate transport failure Error::Request, got: {err:?}"
        );
    }
    #[tokio::test]
    async fn test_mock_login_with_captcha_success_flow() {
        let tv_calls = Arc::new(AtomicUsize::new(0));
        let tv_calls_clone = Arc::clone(&tv_calls);

        let (tv_url, _tv_shutdown) = spawn_mock_server(move |req| {
            let call = tv_calls_clone.fetch_add(1, Ordering::SeqCst);
            if req.path == "/accounts/signin/" {
                let body_str = String::from_utf8_lossy(&req.body);
                if call == 0 {
                    assert!(body_str.contains("username=alice"));
                    assert!(body_str.contains("password=secret"));
                    assert!(!body_str.contains("g-recaptcha-response-v2"));
                    MockServerResponse::json(
                        200,
                        r#"{"code":"recaptcha_required","error":"recaptcha_required"}"#,
                    )
                    .with_cookie("device_t=device_alpha; Path=/")
                    .with_cookie("opaque_cookie=opaque_beta; Path=/")
                } else if call == 1 {
                    assert!(body_str.contains("g-recaptcha-response-v2=solved_token_xyz"));
                    let cookie_hdr = req.header("cookie").expect("cookie header in retry");
                    assert!(cookie_hdr.contains("device_t=device_alpha"));
                    assert!(cookie_hdr.contains("opaque_cookie=opaque_beta"));
                    MockServerResponse::json(
                        200,
                        r#"{"user":{"id":42,"username":"alice","private_channel":"p1","auth_token":"a1","session_hash":"h1"},"error":""}"#,
                    )
                    .with_cookie("sessionid=sess_final; Path=/")
                    .with_cookie("sessionid_sign=sign_final; Path=/")
                } else {
                    panic!("unexpected extra signin call: {call}");
                }
            } else {
                panic!("unexpected path: {}", req.path);
            }
        })
        .await;

        let captcha_create_calls = Arc::new(AtomicUsize::new(0));
        let captcha_poll_calls = Arc::new(AtomicUsize::new(0));
        let captcha_report_calls = Arc::new(AtomicUsize::new(0));

        let c_create = Arc::clone(&captcha_create_calls);
        let c_poll = Arc::clone(&captcha_poll_calls);
        let c_report = Arc::clone(&captcha_report_calls);

        let (captcha_url, _c_shutdown) = spawn_mock_server(move |req| {
            if req.path == "/createTask" {
                c_create.fetch_add(1, Ordering::SeqCst);
                let body_str = String::from_utf8_lossy(&req.body);
                assert!(body_str.contains(r#""clientKey":"test_key""#));
                assert!(body_str.contains(r#""type":"RecaptchaV2TaskProxyless""#));
                MockServerResponse::json(200, r#"{"errorId":0,"taskId":12345}"#)
            } else if req.path == "/getTaskResult" {
                let poll = c_poll.fetch_add(1, Ordering::SeqCst);
                if poll == 0 {
                    MockServerResponse::json(200, r#"{"errorId":0,"status":"processing"}"#)
                } else {
                    MockServerResponse::json(
                        200,
                        r#"{"errorId":0,"status":"ready","solution":{"gRecaptchaResponse":"solved_token_xyz"}}"#,
                    )
                }
            } else if req.path == "/reportIncorrect" {
                c_report.fetch_add(1, Ordering::SeqCst);
                MockServerResponse::json(200, r#"{"errorId":0,"status":"success"}"#)
            } else {
                panic!("unexpected 2captcha path: {}", req.path);
            }
        })
        .await;

        let solver = TwoCaptcha::for_test(
            "test_key",
            &captcha_url,
            Duration::from_millis(5),
            Duration::from_secs(5),
        );

        let client = test_client();
        let mut user = UserCookies::new();
        let logged_in = user
            .login_orchestrated(&client, &tv_url, "alice", "secret", None, Some(&solver))
            .await
            .expect("login with captcha should succeed");

        assert_eq!(logged_in.id, 42);
        assert_eq!(logged_in.username, "alice");
        assert_eq!(logged_in.session, "sess_final");
        assert_eq!(logged_in.session_signature, "sign_final");
        assert_eq!(logged_in.device_token, "device_alpha");

        assert_eq!(tv_calls.load(Ordering::SeqCst), 2);
        assert_eq!(captcha_create_calls.load(Ordering::SeqCst), 1);
        assert_eq!(captcha_report_calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn test_mock_login_with_captcha_second_challenge_reports_incorrect_and_fails() {
        let tv_calls = Arc::new(AtomicUsize::new(0));
        let tv_calls_clone = Arc::clone(&tv_calls);

        let (tv_url, _tv_shutdown) = spawn_mock_server(move |req| {
            tv_calls_clone.fetch_add(1, Ordering::SeqCst);
            if req.path == "/accounts/signin/" {
                MockServerResponse::json(
                    200,
                    r#"{"code":"recaptcha_required","error":"recaptcha_required"}"#,
                )
            } else {
                panic!("unexpected path: {}", req.path);
            }
        })
        .await;

        let captcha_create_calls = Arc::new(AtomicUsize::new(0));
        let captcha_report_calls = Arc::new(AtomicUsize::new(0));
        let c_create = Arc::clone(&captcha_create_calls);
        let c_report = Arc::clone(&captcha_report_calls);

        let (captcha_url, _c_shutdown) = spawn_mock_server(move |req| {
            if req.path == "/createTask" {
                c_create.fetch_add(1, Ordering::SeqCst);
                MockServerResponse::json(200, r#"{"errorId":0,"taskId":54321}"#)
            } else if req.path == "/getTaskResult" {
                MockServerResponse::json(
                    200,
                    r#"{"errorId":0,"status":"ready","solution":{"gRecaptchaResponse":"bad_token"}}"#,
                )
            } else if req.path == "/reportIncorrect" {
                c_report.fetch_add(1, Ordering::SeqCst);
                let body_str = String::from_utf8_lossy(&req.body);
                assert!(body_str.contains(r#""taskId":54321"#));
                assert!(body_str.contains(r#""clientKey":"test_key""#));
                MockServerResponse::json(200, r#"{"errorId":0,"status":"success"}"#)
            } else {
                panic!("unexpected 2captcha path: {}", req.path);
            }
        })
        .await;

        let solver = TwoCaptcha::for_test(
            "test_key",
            &captcha_url,
            Duration::from_millis(5),
            Duration::from_secs(5),
        );

        let client = test_client();
        let mut user = UserCookies::new();
        let err = user
            .login_orchestrated(&client, &tv_url, "alice", "secret", None, Some(&solver))
            .await
            .unwrap_err();

        assert!(
            matches!(
                err,
                Error::Login {
                    source: LoginError::CaptchaRequired
                }
            ),
            "expected CaptchaRequired on double challenge, got: {err:?}"
        );

        assert_eq!(tv_calls.load(Ordering::SeqCst), 2);
        assert_eq!(captcha_create_calls.load(Ordering::SeqCst), 1);
        assert_eq!(captcha_report_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_mock_login_with_captcha_second_challenge_report_failure_propagates_error() {
        let tv_calls = Arc::new(AtomicUsize::new(0));
        let tv_calls_clone = Arc::clone(&tv_calls);

        let (tv_url, _tv_shutdown) = spawn_mock_server(move |_| {
            tv_calls_clone.fetch_add(1, Ordering::SeqCst);
            MockServerResponse::json(
                200,
                r#"{"code":"recaptcha_required","error":"recaptcha_required"}"#,
            )
        })
        .await;

        let captcha_create_calls = Arc::new(AtomicUsize::new(0));
        let captcha_report_calls = Arc::new(AtomicUsize::new(0));
        let c_create = Arc::clone(&captcha_create_calls);
        let c_report = Arc::clone(&captcha_report_calls);

        let (captcha_url, _c_shutdown) = spawn_mock_server(move |req| {
            if req.path == "/createTask" {
                c_create.fetch_add(1, Ordering::SeqCst);
                MockServerResponse::json(200, r#"{"errorId":0,"taskId":999}"#)
            } else if req.path == "/getTaskResult" {
                MockServerResponse::json(
                    200,
                    r#"{"errorId":0,"status":"ready","solution":{"gRecaptchaResponse":"bad_token"}}"#,
                )
            } else if req.path == "/reportIncorrect" {
                c_report.fetch_add(1, Ordering::SeqCst);
                MockServerResponse::json(
                    200,
                    r#"{"errorId":1,"errorCode":"ERROR_REPORT_NOT_RECORDED","errorDescription":"Reporting failed"}"#,
                )
            } else {
                panic!("unexpected 2captcha path: {}", req.path);
            }
        })
        .await;

        let solver = TwoCaptcha::for_test(
            "test_key",
            &captcha_url,
            Duration::from_millis(5),
            Duration::from_secs(5),
        );

        let client = test_client();
        let mut user = UserCookies::new();
        let err = user
            .login_orchestrated(&client, &tv_url, "alice", "secret", None, Some(&solver))
            .await
            .unwrap_err();

        assert!(
            matches!(err, Error::Request(_)),
            "report failure must propagate Error::Request, got: {err:?}"
        );

        assert_eq!(tv_calls.load(Ordering::SeqCst), 2);
        assert_eq!(captcha_create_calls.load(Ordering::SeqCst), 1);
        assert_eq!(captcha_report_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_mock_login_with_captcha_second_challenge_5xx_does_not_report() {
        let tv_calls = Arc::new(AtomicUsize::new(0));
        let tv_calls_clone = Arc::clone(&tv_calls);

        let (tv_url, _tv_shutdown) = spawn_mock_server(move |_| {
            let call = tv_calls_clone.fetch_add(1, Ordering::SeqCst);
            if call == 0 {
                MockServerResponse::json(
                    200,
                    r#"{"code":"recaptcha_required","error":"recaptcha_required"}"#,
                )
            } else {
                MockServerResponse::json(500, r#"{"error":"internal_server_error"}"#)
            }
        })
        .await;

        let captcha_report_calls = Arc::new(AtomicUsize::new(0));
        let c_report = Arc::clone(&captcha_report_calls);

        let (captcha_url, _c_shutdown) = spawn_mock_server(move |req| {
            if req.path == "/createTask" {
                MockServerResponse::json(200, r#"{"errorId":0,"taskId":888}"#)
            } else if req.path == "/getTaskResult" {
                MockServerResponse::json(
                    200,
                    r#"{"errorId":0,"status":"ready","solution":{"gRecaptchaResponse":"token_ok"}}"#,
                )
            } else if req.path == "/reportIncorrect" {
                c_report.fetch_add(1, Ordering::SeqCst);
                MockServerResponse::json(200, r#"{"errorId":0,"status":"success"}"#)
            } else {
                panic!("unexpected 2captcha path: {}", req.path);
            }
        })
        .await;

        let solver = TwoCaptcha::for_test(
            "test_key",
            &captcha_url,
            Duration::from_millis(5),
            Duration::from_secs(5),
        );

        let client = test_client();
        let mut user = UserCookies::new();
        let err = user
            .login_orchestrated(&client, &tv_url, "alice", "secret", None, Some(&solver))
            .await
            .unwrap_err();

        assert!(
            matches!(err, Error::Request(_)),
            "HTTP 500 must return Error::Request, got: {err:?}"
        );
        assert_eq!(captcha_report_calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn test_mock_login_with_captcha_rate_limited_http200_does_not_solve() {
        let (tv_url, _tv_shutdown) =
            spawn_mock_server(|_| MockServerResponse::json(200, r#"{"code":"rate_limit"}"#)).await;

        let captcha_calls = Arc::new(AtomicUsize::new(0));
        let c_calls = Arc::clone(&captcha_calls);

        let (captcha_url, _c_shutdown) = spawn_mock_server(move |_| {
            c_calls.fetch_add(1, Ordering::SeqCst);
            MockServerResponse::json(200, r#"{"errorId":0,"taskId":1}"#)
        })
        .await;

        let solver = TwoCaptcha::for_test(
            "test_key",
            &captcha_url,
            Duration::from_millis(5),
            Duration::from_secs(5),
        );

        let client = test_client();
        let mut user = UserCookies::new();
        let err = user
            .login_orchestrated(&client, &tv_url, "alice", "secret", None, Some(&solver))
            .await
            .unwrap_err();

        assert!(
            matches!(err, Error::RateLimited(_)),
            "expected RateLimited on code=rate_limit, got: {err:?}"
        );
        assert_eq!(captcha_calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn test_mock_login_with_captcha_http429_does_not_solve() {
        let (tv_url, _tv_shutdown) =
            spawn_mock_server(|_| MockServerResponse::text(429, "Too Many Requests", "text/plain"))
                .await;

        let captcha_calls = Arc::new(AtomicUsize::new(0));
        let c_calls = Arc::clone(&captcha_calls);

        let (captcha_url, _c_shutdown) = spawn_mock_server(move |_| {
            c_calls.fetch_add(1, Ordering::SeqCst);
            MockServerResponse::json(200, r#"{"errorId":0,"taskId":1}"#)
        })
        .await;

        let solver = TwoCaptcha::for_test(
            "test_key",
            &captcha_url,
            Duration::from_millis(5),
            Duration::from_secs(5),
        );

        let client = test_client();
        let mut user = UserCookies::new();
        let err = user
            .login_orchestrated(&client, &tv_url, "alice", "secret", None, Some(&solver))
            .await
            .unwrap_err();

        assert!(
            matches!(err, Error::RateLimited(_)),
            "expected RateLimited on HTTP 429, got: {err:?}"
        );
        assert_eq!(captcha_calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn test_mock_login_with_captcha_invalid_credentials_does_not_solve() {
        let (tv_url, _tv_shutdown) = spawn_mock_server(|_| {
            MockServerResponse::json(200, r#"{"error":"username or password is not correct"}"#)
        })
        .await;

        let captcha_calls = Arc::new(AtomicUsize::new(0));
        let c_calls = Arc::clone(&captcha_calls);

        let (captcha_url, _c_shutdown) = spawn_mock_server(move |_| {
            c_calls.fetch_add(1, Ordering::SeqCst);
            MockServerResponse::json(200, r#"{"errorId":0,"taskId":1}"#)
        })
        .await;

        let solver = TwoCaptcha::for_test(
            "test_key",
            &captcha_url,
            Duration::from_millis(5),
            Duration::from_secs(5),
        );

        let client = test_client();
        let mut user = UserCookies::new();
        let err = user
            .login_orchestrated(&client, &tv_url, "alice", "wrong_pass", None, Some(&solver))
            .await
            .unwrap_err();

        assert!(
            matches!(
                err,
                Error::Login {
                    source: LoginError::InvalidCredentials
                }
            ),
            "expected InvalidCredentials, got: {err:?}"
        );
        assert_eq!(captcha_calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn test_mock_login_with_captcha_preserves_opaque_cookies_into_mfa() {
        let tv_calls = Arc::new(AtomicUsize::new(0));
        let tv_calls_clone = Arc::clone(&tv_calls);

        let (tv_url, _tv_shutdown) = spawn_mock_server(move |req| {
            let call = tv_calls_clone.fetch_add(1, Ordering::SeqCst);
            if call == 0 {
                MockServerResponse::json(
                    200,
                    r#"{"code":"recaptcha_required","error":"recaptcha_required"}"#,
                )
                .with_cookie("opaque_chal=chal_token; Path=/")
                .with_cookie("device_t=initial_dev; Path=/")
            } else if call == 1 {
                let cookie_hdr = req.header("cookie").expect("cookie header in retry");
                assert!(cookie_hdr.contains("opaque_chal=chal_token"));
                assert!(cookie_hdr.contains("device_t=initial_dev"));
                MockServerResponse::json(200, r#"{"error":"2FA_required"}"#)
                    .with_cookie("sessionid=temp_sess; Path=/")
                    .with_cookie("sessionid_sign=temp_sig; Path=/")
            } else if call == 2 {
                assert_eq!(req.path, "/accounts/two-factor/signin/totp/");
                let cookie_hdr = req.header("cookie").expect("cookie header in MFA");
                assert!(cookie_hdr.contains("opaque_chal=chal_token"));
                assert!(cookie_hdr.contains("device_t=initial_dev"));
                assert!(cookie_hdr.contains("sessionid=temp_sess"));
                assert!(cookie_hdr.contains("sessionid_sign=temp_sig"));
                MockServerResponse::json(
                    200,
                    r#"{"user":{"id":55,"username":"dan","private_channel":"p4","auth_token":"a4","session_hash":"h4"},"error":""}"#,
                )
                .with_cookie("sessionid=rotated_sess; Path=/")
                .with_cookie("sessionid_sign=rotated_sig; Path=/")
            } else {
                panic!("unexpected call: {call}");
            }
        })
        .await;

        let (captcha_url, _c_shutdown) = spawn_mock_server(|req| {
            if req.path == "/createTask" {
                MockServerResponse::json(200, r#"{"errorId":0,"taskId":111}"#)
            } else {
                MockServerResponse::json(
                    200,
                    r#"{"errorId":0,"status":"ready","solution":{"gRecaptchaResponse":"valid_token"}}"#,
                )
            }
        })
        .await;

        let solver = TwoCaptcha::for_test(
            "test_key",
            &captcha_url,
            Duration::from_millis(5),
            Duration::from_secs(5),
        );

        let client = test_client();
        let mut user = UserCookies::new();
        let totp_secret = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";
        let logged_in = user
            .login_orchestrated(
                &client,
                &tv_url,
                "dan",
                "secret",
                Some(totp_secret),
                Some(&solver),
            )
            .await
            .expect("login with MFA after captcha should succeed");

        assert_eq!(logged_in.id, 55);
        assert_eq!(logged_in.username, "dan");
        assert_eq!(logged_in.session, "rotated_sess");
        assert_eq!(logged_in.session_signature, "rotated_sig");
        assert_eq!(logged_in.device_token, "initial_dev");
        assert_eq!(tv_calls.load(Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_mock_login_with_captcha_mfa_recaptcha_returns_captcha_required_no_solve() {
        let tv_calls = Arc::new(AtomicUsize::new(0));
        let tv_calls_clone = Arc::clone(&tv_calls);

        let (tv_url, _tv_shutdown) = spawn_mock_server(move |_req| {
            let call = tv_calls_clone.fetch_add(1, Ordering::SeqCst);
            if call == 0 {
                MockServerResponse::json(
                    200,
                    r#"{"code":"recaptcha_required","error":"recaptcha_required"}"#,
                )
            } else if call == 1 {
                MockServerResponse::json(200, r#"{"error":"2FA_required"}"#)
                    .with_cookie("sessionid=temp_sess; Path=/")
                    .with_cookie("sessionid_sign=temp_sig; Path=/")
            } else if call == 2 {
                MockServerResponse::json(200, r#"{"code":"recaptcha_required"}"#)
            } else {
                panic!("unexpected call: {call}");
            }
        })
        .await;

        let captcha_create_calls = Arc::new(AtomicUsize::new(0));
        let c_create = Arc::clone(&captcha_create_calls);

        let (captcha_url, _c_shutdown) = spawn_mock_server(move |req| {
            if req.path == "/createTask" {
                c_create.fetch_add(1, Ordering::SeqCst);
                MockServerResponse::json(200, r#"{"errorId":0,"taskId":222}"#)
            } else {
                MockServerResponse::json(
                    200,
                    r#"{"errorId":0,"status":"ready","solution":{"gRecaptchaResponse":"valid_token"}}"#,
                )
            }
        })
        .await;

        let solver = TwoCaptcha::for_test(
            "test_key",
            &captcha_url,
            Duration::from_millis(5),
            Duration::from_secs(5),
        );

        let client = test_client();
        let mut user = UserCookies::new();
        let totp_secret = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";
        let err = user
            .login_orchestrated(
                &client,
                &tv_url,
                "dan",
                "secret",
                Some(totp_secret),
                Some(&solver),
            )
            .await
            .unwrap_err();

        assert!(
            matches!(
                err,
                Error::Login {
                    source: LoginError::CaptchaRequired
                }
            ),
            "MFA captcha must return CaptchaRequired without solving, got: {err:?}"
        );
        // Exactly 1 createTask on initial signin, 0 during MFA!
        assert_eq!(captcha_create_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_mock_login_with_captcha_wrong_credentials_after_solve_does_not_report() {
        let (tv_url, _tv_shutdown) = spawn_mock_server(|req| {
            let body_str = String::from_utf8_lossy(&req.body);
            if body_str.contains("g-recaptcha-response-v2") {
                MockServerResponse::json(200, r#"{"error":"invalid_credentials"}"#)
            } else {
                MockServerResponse::json(
                    200,
                    r#"{"code":"recaptcha_required","error":"recaptcha_required"}"#,
                )
            }
        })
        .await;

        let captcha_report_calls = Arc::new(AtomicUsize::new(0));
        let c_report = Arc::clone(&captcha_report_calls);

        let (captcha_url, _c_shutdown) = spawn_mock_server(move |req| {
            if req.path == "/createTask" {
                MockServerResponse::json(200, r#"{"errorId":0,"taskId":333}"#)
            } else if req.path == "/getTaskResult" {
                MockServerResponse::json(
                    200,
                    r#"{"errorId":0,"status":"ready","solution":{"gRecaptchaResponse":"token"}}"#,
                )
            } else if req.path == "/reportIncorrect" {
                c_report.fetch_add(1, Ordering::SeqCst);
                MockServerResponse::json(200, r#"{"errorId":0,"status":"success"}"#)
            } else {
                panic!("unexpected 2captcha path: {}", req.path);
            }
        })
        .await;

        let solver = TwoCaptcha::for_test(
            "test_key",
            &captcha_url,
            Duration::from_millis(5),
            Duration::from_secs(5),
        );

        let client = test_client();
        let mut user = UserCookies::new();
        let err = user
            .login_orchestrated(&client, &tv_url, "alice", "wrong_pass", None, Some(&solver))
            .await
            .unwrap_err();

        assert!(
            matches!(
                err,
                Error::Login {
                    source: LoginError::InvalidCredentials
                }
            ),
            "expected InvalidCredentials, got: {err:?}"
        );
        assert_eq!(captcha_report_calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn test_login_with_captcha_blank_api_key_and_empty_credentials_offline() {
        let mut user = UserCookies::new();

        let err_blank_key = user
            .login_with_captcha("alice", "secret", None, "   ")
            .await
            .unwrap_err();
        assert!(
            matches!(err_blank_key, Error::Request(_)),
            "expected Error::Request on blank key, got: {err_blank_key:?}"
        );

        let err_empty_user = user
            .login_with_captcha("", "secret", None, "test_key")
            .await
            .unwrap_err();
        assert!(
            matches!(
                err_empty_user,
                Error::Login {
                    source: LoginError::EmptyCredentials
                }
            ),
            "expected EmptyCredentials on empty username, got: {err_empty_user:?}"
        );

        let err_empty_pass = user
            .login_with_captcha("alice", "", None, "test_key")
            .await
            .unwrap_err();
        assert!(
            matches!(
                err_empty_pass,
                Error::Login {
                    source: LoginError::EmptyCredentials
                }
            ),
            "expected EmptyCredentials on empty password, got: {err_empty_pass:?}"
        );
    }

    #[tokio::test]
    async fn test_mock_login_with_captcha_http500_with_captcha_initial_does_not_solve() {
        let (tv_url, _tv_shutdown) = spawn_mock_server(|_| {
            MockServerResponse::json(
                500,
                r#"{"code":"recaptcha_required","error":"server_error"}"#,
            )
        })
        .await;

        let captcha_calls = Arc::new(AtomicUsize::new(0));
        let c_calls = Arc::clone(&captcha_calls);

        let (captcha_url, _c_shutdown) = spawn_mock_server(move |_| {
            c_calls.fetch_add(1, Ordering::SeqCst);
            MockServerResponse::json(200, r#"{"errorId":0,"taskId":1}"#)
        })
        .await;

        let solver = TwoCaptcha::for_test(
            "test_key",
            &captcha_url,
            Duration::from_millis(5),
            Duration::from_secs(5),
        );

        let client = test_client();
        let mut user = UserCookies::new();
        let err = user
            .login_orchestrated(&client, &tv_url, "alice", "secret", None, Some(&solver))
            .await
            .unwrap_err();

        assert!(
            matches!(err, Error::Request(_)),
            "HTTP 500 on initial signin must return Error::Request, got: {err:?}"
        );
        assert_eq!(captcha_calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn test_mock_login_with_captcha_http500_with_captcha_retry_does_not_report() {
        let tv_calls = Arc::new(AtomicUsize::new(0));
        let tv_calls_clone = Arc::clone(&tv_calls);

        let (tv_url, _tv_shutdown) = spawn_mock_server(move |_| {
            let call = tv_calls_clone.fetch_add(1, Ordering::SeqCst);
            if call == 0 {
                MockServerResponse::json(
                    200,
                    r#"{"code":"recaptcha_required","error":"recaptcha_required"}"#,
                )
            } else {
                MockServerResponse::json(
                    500,
                    r#"{"code":"recaptcha_required","error":"internal_server_error"}"#,
                )
            }
        })
        .await;

        let captcha_report_calls = Arc::new(AtomicUsize::new(0));
        let c_report = Arc::clone(&captcha_report_calls);

        let (captcha_url, _c_shutdown) = spawn_mock_server(move |req| {
            if req.path == "/createTask" {
                MockServerResponse::json(200, r#"{"errorId":0,"taskId":999}"#)
            } else if req.path == "/getTaskResult" {
                MockServerResponse::json(
                    200,
                    r#"{"errorId":0,"status":"ready","solution":{"gRecaptchaResponse":"token"}}"#,
                )
            } else if req.path == "/reportIncorrect" {
                c_report.fetch_add(1, Ordering::SeqCst);
                MockServerResponse::json(200, r#"{"errorId":0,"status":"success"}"#)
            } else {
                panic!("unexpected 2captcha path: {}", req.path);
            }
        })
        .await;

        let solver = TwoCaptcha::for_test(
            "test_key",
            &captcha_url,
            Duration::from_millis(5),
            Duration::from_secs(5),
        );

        let client = test_client();
        let mut user = UserCookies::new();
        let err = user
            .login_orchestrated(&client, &tv_url, "alice", "secret", None, Some(&solver))
            .await
            .unwrap_err();

        assert!(
            matches!(err, Error::Request(_)),
            "HTTP 500 on retry signin must return Error::Request, got: {err:?}"
        );
        assert_eq!(captcha_report_calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn test_mock_login_wrong_path_session_cookie_rejected() {
        let (base_url, _shutdown) = spawn_mock_server(|_| {
            MockServerResponse::json(
                200,
                r#"{"user":{"id":42,"username":"alice","private_channel":"p1","auth_token":"a1","session_hash":"h1"},"error":""}"#,
            )
            .with_cookie("sessionid=sess_scoped; Path=/accounts/signin/")
            .with_cookie("sessionid_sign=sig_scoped; Path=/accounts/signin/")
            .with_cookie("device_t=dev_scoped; Path=/accounts/signin/")
        })
        .await;

        let client = test_client();
        let mut user = UserCookies::new();
        let err = user
            .login_internal(&client, &base_url, "alice", "secret", None)
            .await
            .unwrap_err();

        assert!(
            matches!(
                err,
                Error::Login {
                    source: LoginError::SessionNotFound
                }
            ),
            "cookies with path restricted away from quote_token must be rejected with SessionNotFound, got: {err:?}"
        );
    }
}
