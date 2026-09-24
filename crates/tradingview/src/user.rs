pub use crate::models::UserCookies;
use crate::{
    Result,
    error::{Error, LoginError},
    utils::http_client,
};
use serde::Deserialize;
use serde_json::Value;
use totp_rs::{Builder, Secret, Totp, TotpError};
use tracing::{error, info, warn};
use wreq::{Response, header::COOKIE};

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
        let client = http_client();
        let response = client
            .post("https://www.tradingview.com/accounts/signin/")
            .form(&[
                ("username", username),
                ("password", password),
                ("remember", "true"),
            ])
            .send()
            .await?;

        let mut session: Option<String> = None;
        let mut signature: Option<String> = None;
        let mut device_token: Option<String> = None;

        for cookie in response.cookies() {
            match cookie.name() {
                "sessionid" => session = Some(cookie.value().to_string()),
                "sessionid_sign" => signature = Some(cookie.value().to_string()),
                "device_t" => device_token = Some(cookie.value().to_string()),
                _ => {}
            }
        }

        #[derive(Debug, Deserialize)]
        struct LoginUserResponse {
            user: UserCookies,
        }

        let body: Value = response.json().await?;

        if is_recaptcha_required(&body) {
            return Err(Error::Login {
                source: LoginError::CaptchaRequired,
            });
        }
        let err_str = body.get("error").and_then(|v| v.as_str()).unwrap_or("");

        if session.is_none() || signature.is_none() {
            error!("unable to login, username or password is invalid");
            return Err(Error::Login {
                source: LoginError::InvalidCredentials,
            });
        }

        if err_str.is_empty() {
            warn!("2FA is not enabled for this account");
            info!("User is logged in successfully");
            let login_resp: LoginUserResponse = serde_json::from_value(body)?;

            Ok(UserCookies {
                session: session.unwrap_or_default(),
                session_signature: signature.unwrap_or_default(),
                device_token: device_token.unwrap_or_default(),
                ..login_resp.user
            })
        } else if err_str == "2FA_required" {
            if totp_secret.is_none() {
                error!("2FA is enabled for this account, but no TOTP secret was provided");
                return Err(Error::Login {
                    source: LoginError::OTPSecretNotFound,
                });
            }

            let mfa_response = Self::handle_mfa(
                totp_secret.unwrap(),
                session.as_deref().unwrap_or_default(),
                signature.as_deref().unwrap_or_default(),
            )
            .await?;

            let mut mfa_session: Option<String> = None;
            let mut mfa_signature: Option<String> = None;
            let mut mfa_device_token: Option<String> = None;

            for cookie in mfa_response.cookies() {
                match cookie.name() {
                    "sessionid" => mfa_session = Some(cookie.value().to_string()),
                    "sessionid_sign" => mfa_signature = Some(cookie.value().to_string()),
                    "device_t" => mfa_device_token = Some(cookie.value().to_string()),
                    _ => {}
                }
            }

            let final_session = mfa_session.or(session).unwrap_or_default();
            let final_signature = mfa_signature.or(signature).unwrap_or_default();
            let final_device_token = mfa_device_token.or(device_token).unwrap_or_default();

            let mfa_body: Value = mfa_response.json().await?;
            let login_resp: LoginUserResponse = serde_json::from_value(mfa_body)?;

            info!("2FA authentication completed");
            info!("User is logged in successfully");

            Ok(UserCookies {
                session: final_session,
                session_signature: final_signature,
                device_token: final_device_token,
                ..login_resp.user
            })
        } else {
            error!("unable to login, username or password is invalid");
            Err(Error::Login {
                source: LoginError::InvalidCredentials,
            })
        }
    }

    async fn handle_mfa(totp_secret: &str, session: &str, signature: &str) -> Result<Response> {
        let trimmed = totp_secret.trim();
        if trimmed.is_empty() {
            return Err(Error::Login {
                source: LoginError::OTPSecretNotFound,
            });
        }

        let code = generate_totp_code(trimmed).map_err(|_| {
            error!("invalid TOTP configuration");
            Error::Login {
                source: LoginError::InvalidOTPSecret,
            }
        })?;

        let cookie = format!("sessionid={session}; sessionid_sign={signature};");
        let response = http_client()
            .post("https://www.tradingview.com/accounts/two-factor/signin/totp/")
            .header(COOKIE, &cookie)
            .form(&[("code", code.as_str())])
            .send()
            .await?;

        if response.status().is_success() {
            Ok(response)
        } else {
            Err(Error::Login {
                source: LoginError::InvalidOTPSecret,
            })
        }
    }
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
        let sec = Secret::try_from_base32(trimmed).map_err(|_| TotpError::InvalidSecret)?;
        let builder = Builder::new().with_secret(sec);
        match builder.build() {
            Ok(totp) => Ok(totp),
            Err(TotpError::SecretTooShort { .. }) => {
                let sec = Secret::try_from_base32(trimmed).map_err(|_| TotpError::InvalidSecret)?;
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
}
