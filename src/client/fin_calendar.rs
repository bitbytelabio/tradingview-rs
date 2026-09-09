//! Economic calendar REST API client.
//!
//! TradingView provides an economic calendar endpoint returning scheduled
//! macroeconomic events, speeches, auctions, and indicators across countries.
//!
//! # Endpoint
//!
//! `GET https://economic-calendar.tradingview.com/events`
//!
//! # Example
//!
//! ```no_run
//! use chrono::{Duration, Utc};
//! use tradingview::client::fin_calendar::{
//!     EconomicCalendarRequest, EconomicImportance, get_economic_calendar,
//! };
//!
//! # async fn run() -> tradingview::Result<()> {
//! let now = Utc::now();
//! let request = EconomicCalendarRequest::builder()
//!     .from(now)
//!     .to(now + Duration::days(7))
//!     .countries(vec!["US".to_string(), "DE".to_string()])
//!     .min_importance(EconomicImportance::Medium)
//!     .build();
//!
//! let events = get_economic_calendar(&request).await?;
//! for event in events {
//!     println!("{}: {} ({:?})", event.date, event.title, event.importance_level());
//! }
//! # Ok(())
//! # }
//! ```

use std::{borrow::Borrow, sync::Arc};

use bon::Builder;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use ustr::Ustr;

use crate::{Error, Result, client::core::DataClient, utils::http_client};

/// Default endpoint URL for TradingView's economic calendar events.
pub static ECONOMIC_CALENDAR_URL: &str = "https://economic-calendar.tradingview.com/events";

/// Macroeconomic event importance level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(i32)]
pub enum EconomicImportance {
    /// Low / minor importance (-1).
    Low = -1,
    /// Medium / moderate importance (0).
    Medium = 0,
    /// High / major importance (1).
    High = 1,
}

impl std::fmt::Display for EconomicImportance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Low => write!(f, "Low"),
            Self::Medium => write!(f, "Medium"),
            Self::High => write!(f, "High"),
        }
    }
}

impl From<EconomicImportance> for i32 {
    #[inline]
    fn from(importance: EconomicImportance) -> Self {
        importance as i32
    }
}

impl TryFrom<i32> for EconomicImportance {
    type Error = Error;

    fn try_from(value: i32) -> Result<Self> {
        match value {
            -1 => Ok(Self::Low),
            0 => Ok(Self::Medium),
            1 => Ok(Self::High),
            other => Err(Error::Internal(Ustr::from(&format!(
                "unknown economic importance value: {other} (expected -1, 0, or 1)"
            )))),
        }
    }
}

/// Request parameters for querying TradingView's economic calendar.
#[derive(Debug, Clone, Builder, PartialEq)]
pub struct EconomicCalendarRequest {
    /// Start of date range (inclusive, UTC).
    pub from: DateTime<Utc>,

    /// End of date range (inclusive, UTC).
    pub to: DateTime<Utc>,

    /// Optional list of ISO 3166-1 alpha-2 country codes (e.g. `["US", "DE"]`).
    #[builder(default)]
    pub countries: Vec<String>,

    /// Optional minimum importance filter applied client-side.
    pub min_importance: Option<EconomicImportance>,
}

impl EconomicCalendarRequest {
    /// Creates a new request for the specified UTC date range.
    pub fn new(from: DateTime<Utc>, to: DateTime<Utc>) -> Self {
        Self {
            from,
            to,
            countries: Vec::new(),
            min_importance: None,
        }
    }

    /// Validates request parameters without network interaction.
    pub fn validate(&self) -> Result<()> {
        if self.from > self.to {
            return Err(Error::Internal(Ustr::from(&format!(
                "invalid date range: 'from' ({}) cannot be greater than 'to' ({})",
                self.from, self.to
            ))));
        }
        for code in &self.countries {
            validate_country_code(code)?;
        }
        Ok(())
    }

    /// Returns canonical uppercase ISO 3166-1 alpha-2 country codes.
    pub fn canonical_country_codes(&self) -> Result<Vec<String>> {
        self.countries
            .iter()
            .map(|c| validate_country_code(c))
            .collect()
    }

    /// Checks whether an event passes the configured importance filter.
    pub fn matches_filter(&self, event: &EconomicCalendarEvent) -> bool {
        match self.min_importance {
            Some(min) => event.importance >= min as i32,
            None => true,
        }
    }

    /// Sends this request using the shared [`http_client`].
    pub async fn send(&self) -> Result<Vec<EconomicCalendarEvent>> {
        get_economic_calendar(self).await
    }
}

/// Validates and canonicalizes an ISO 3166-1 alpha-2 country code.
pub fn validate_country_code(code: &str) -> Result<String> {
    let trimmed = code.trim();
    if trimmed.len() != 2 || !trimmed.chars().all(|c| c.is_ascii_alphabetic()) {
        return Err(Error::Internal(Ustr::from(&format!(
            "invalid iso alpha-2 country code '{code}': expected 2 ascii letters"
        ))));
    }
    Ok(trimmed.to_ascii_uppercase())
}

/// Builds the query parameter list for the economic calendar endpoint.
pub fn build_query_params(
    request: &EconomicCalendarRequest,
) -> Result<Vec<(&'static str, String)>> {
    request.validate()?;
    let from_str = request
        .from
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let to_str = request
        .to
        .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);

    let mut params = vec![("from", from_str), ("to", to_str)];

    let canonical = request.canonical_country_codes()?;
    if !canonical.is_empty() {
        params.push(("countries", canonical.join(",")));
    }

    Ok(params)
}

/// A single macroeconomic calendar event from TradingView.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EconomicCalendarEvent {
    /// Unique event identifier (e.g. `"367073"`).
    pub id: String,

    /// Human-readable title of the event.
    pub title: String,

    /// ISO 3166-1 alpha-2 country code.
    pub country: String,

    /// Indicator name.
    pub indicator: String,

    /// TradingView ticker symbol if available.
    #[serde(default)]
    pub ticker: Option<String>,

    /// Descriptive commentary or context about the metric.
    #[serde(default)]
    pub comment: Option<String>,

    /// Category code (e.g. `"lbr"`, `"mny"`, `"hse"`).
    #[serde(default)]
    pub category: Option<String>,

    /// Reporting period string (e.g. `"Dec"`).
    #[serde(default)]
    pub period: String,

    /// Reference date for the data release, if applicable.
    #[serde(
        rename = "referenceDate",
        default,
        deserialize_with = "deserialize_optional_datetime"
    )]
    pub reference_date: Option<DateTime<Utc>>,

    /// Data source organization.
    #[serde(default)]
    pub source: String,

    /// URL to the official source website.
    #[serde(rename = "source_url", default)]
    pub source_url: String,

    /// Actual released value.
    #[serde(default)]
    pub actual: Option<f64>,

    /// Previous period's value.
    #[serde(default)]
    pub previous: Option<f64>,

    /// Market consensus forecast value.
    #[serde(default)]
    pub forecast: Option<f64>,

    /// Raw unscaled actual value.
    #[serde(rename = "actualRaw", default)]
    pub actual_raw: Option<f64>,

    /// Raw unscaled previous value.
    #[serde(rename = "previousRaw", default)]
    pub previous_raw: Option<f64>,

    /// Raw unscaled forecast value.
    #[serde(rename = "forecastRaw", default)]
    pub forecast_raw: Option<f64>,

    /// Currency associated with the event.
    #[serde(default)]
    pub currency: String,

    /// Display unit of measurement.
    #[serde(default)]
    pub unit: Option<String>,

    /// Scale multiplier (e.g. `"K"`, `"M"`, `"B"`, `"T"`).
    #[serde(default)]
    pub scale: Option<String>,

    /// Importance level: -1 (Low), 0 (Medium), 1 (High).
    pub importance: i32,

    /// Scheduled release timestamp in UTC.
    pub date: DateTime<Utc>,
}

impl EconomicCalendarEvent {
    /// Returns the importance as a typed [`EconomicImportance`] if recognized.
    pub fn importance_level(&self) -> Option<EconomicImportance> {
        EconomicImportance::try_from(self.importance).ok()
    }
}

#[derive(Debug, Deserialize)]
struct EconomicCalendarResponse {
    status: String,
    #[serde(default)]
    result: Option<Vec<EconomicCalendarEvent>>,
    #[serde(default)]
    errmsg: Option<String>,
    #[serde(default)]
    message: Option<String>,
}

fn deserialize_optional_datetime<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<DateTime<Utc>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    match Option::<String>::deserialize(deserializer)? {
        Some(s) if !s.trim().is_empty() => DateTime::parse_from_rfc3339(&s)
            .map(|dt| Some(dt.with_timezone(&Utc)))
            .map_err(serde::de::Error::custom),
        _ => Ok(None),
    }
}

/// Fetches economic calendar events from TradingView using the shared [`http_client`].
pub async fn get_economic_calendar(
    request: impl Borrow<EconomicCalendarRequest>,
) -> Result<Vec<EconomicCalendarEvent>> {
    let client = http_client();
    get_economic_calendar_with_client(&client, request.borrow()).await
}

/// Fetches economic calendar events using the specified [`reqwest::Client`].
pub async fn get_economic_calendar_with_client(
    client: &reqwest::Client,
    request: &EconomicCalendarRequest,
) -> Result<Vec<EconomicCalendarEvent>> {
    let query_params = build_query_params(request)?;

    let response = client
        .get(ECONOMIC_CALENDAR_URL)
        .query(&query_params)
        .send()
        .await
        .map_err(|e| {
            Error::Request(Ustr::from(&format!(
                "economic calendar request failed: {e}"
            )))
        })?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.map_err(|e| {
            Error::Request(Ustr::from(&format!("failed to read response body: {e}")))
        })?;
        return Err(Error::Request(Ustr::from(&format!(
            "economic calendar request failed with status {status}: {body}"
        ))));
    }

    let parsed = response
        .json::<EconomicCalendarResponse>()
        .await
        .map_err(|e| {
            Error::JsonParse(Ustr::from(&format!(
                "failed to parse economic calendar response: {e}"
            )))
        })?;

    if parsed.status != "ok" {
        let msg = parsed
            .errmsg
            .as_deref()
            .or(parsed.message.as_deref())
            .unwrap_or("unknown error");
        return Err(Error::Request(Ustr::from(&format!(
            "tradingview economic calendar error (status '{}'): {msg}",
            parsed.status
        ))));
    }

    let events = parsed.result.unwrap_or_default();
    Ok(events
        .into_iter()
        .filter(|ev| request.matches_filter(ev))
        .collect())
}

/// Client for TradingView's Economic Calendar REST API.
#[derive(Debug, Default, Clone)]
pub struct EconomicCalendarClient {
    client: reqwest::Client,
}

impl EconomicCalendarClient {
    /// Creates a new client reusing the crate's shared [`http_client`].
    pub fn new() -> Self {
        Self {
            client: http_client(),
        }
    }

    /// Creates a new client with a custom [`reqwest::Client`].
    pub fn with_client(client: reqwest::Client) -> Self {
        Self { client }
    }

    /// Fetches economic calendar events for the given request.
    pub async fn get_events(
        &self,
        request: impl Borrow<EconomicCalendarRequest>,
    ) -> Result<Vec<EconomicCalendarEvent>> {
        get_economic_calendar_with_client(&self.client, request.borrow()).await
    }
}

impl DataClient for EconomicCalendarClient {
    fn new(_auth_token: Option<&str>) -> Arc<Self> {
        Arc::new(Self::new())
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    #[test]
    fn test_validation() {
        assert_eq!(validate_country_code("US").unwrap(), "US");
        assert_eq!(validate_country_code("de").unwrap(), "DE");
        assert!(validate_country_code("USA").is_err());
        assert!(validate_country_code("12").is_err());

        let t1 = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
        let t2 = Utc.with_ymd_and_hms(2025, 1, 2, 0, 0, 0).unwrap();

        assert!(EconomicCalendarRequest::new(t1, t2).validate().is_ok());
        assert!(EconomicCalendarRequest::new(t2, t1).validate().is_err());
    }

    #[test]
    fn test_query_params_and_encoding() {
        let from = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
        let to = Utc.with_ymd_and_hms(2025, 1, 2, 0, 0, 0).unwrap();

        let req = EconomicCalendarRequest::builder()
            .from(from)
            .to(to)
            .countries(vec!["us".to_string(), "DE".to_string()])
            .build();

        let params = build_query_params(&req).unwrap();
        assert_eq!(
            params,
            vec![
                ("from", "2025-01-01T00:00:00.000Z".to_string()),
                ("to", "2025-01-02T00:00:00.000Z".to_string()),
                ("countries", "US,DE".to_string()),
            ]
        );

        let url = reqwest::Client::new()
            .get(ECONOMIC_CALENDAR_URL)
            .query(&params)
            .build()
            .unwrap()
            .url()
            .to_string();

        assert!(url.contains("from=2025-01-01T00%3A00%3A00.000Z"));
        assert!(url.contains("countries=US%2CDE"));
    }

    #[test]
    fn test_deserialize_observed_json() {
        let json_data = r#"{
            "status": "ok",
            "result": [
                {
                    "id": "371705",
                    "title": "Inflation Rate YoY Prel",
                    "country": "DE",
                    "indicator": "Inflation Rate",
                    "ticker": "ECONOMICS:DEIRYY",
                    "comment": "Commentary",
                    "category": "prce",
                    "period": "Dec",
                    "referenceDate": "2024-12-31T00:00:00Z",
                    "source": "Federal Statistical Office",
                    "source_url": "https://www.destatis.de",
                    "actual": 2.6,
                    "previous": 2.2,
                    "forecast": 2.4,
                    "actualRaw": 2.6,
                    "previousRaw": 2.2,
                    "forecastRaw": 2.4,
                    "currency": "EUR",
                    "unit": "%",
                    "importance": 1,
                    "date": "2025-01-06T13:00:00.000Z"
                },
                {
                    "id": "367073",
                    "title": "New Year’s Day",
                    "country": "US",
                    "indicator": "Holidays",
                    "period": "",
                    "referenceDate": null,
                    "source": "",
                    "source_url": "",
                    "actual": null,
                    "previous": null,
                    "forecast": null,
                    "actualRaw": null,
                    "previousRaw": null,
                    "forecastRaw": null,
                    "currency": "USD",
                    "importance": -1,
                    "date": "2025-01-01T00:00:00.000Z"
                }
            ]
        }"#;

        let resp: EconomicCalendarResponse = serde_json::from_str(json_data).unwrap();
        let events = resp.result.unwrap();
        assert_eq!(events.len(), 2);

        assert_eq!(events[0].id, "371705");
        assert_eq!(events[0].actual, Some(2.6));
        assert_eq!(events[0].importance_level(), Some(EconomicImportance::High));
        assert_eq!(
            events[0].reference_date,
            Some(Utc.with_ymd_and_hms(2024, 12, 31, 0, 0, 0).unwrap())
        );

        assert_eq!(events[1].id, "367073");
        assert_eq!(events[1].actual, None);
        assert_eq!(events[1].reference_date, None);
        assert_eq!(events[1].importance_level(), Some(EconomicImportance::Low));
    }

    #[test]
    fn test_error_response_rejection() {
        let json_err = r#"{"status": "bad_request", "errmsg": "parse error"}"#;
        let resp: EconomicCalendarResponse = serde_json::from_str(json_err).unwrap();
        assert_eq!(resp.status, "bad_request");
        assert_eq!(resp.errmsg.as_deref(), Some("parse error"));
    }

    #[test]
    fn test_importance_client_side_filtering() {
        let make_event = |importance: i32| EconomicCalendarEvent {
            id: "1".to_string(),
            title: "T".to_string(),
            country: "US".to_string(),
            indicator: "I".to_string(),
            ticker: None,
            comment: None,
            category: None,
            period: "".to_string(),
            reference_date: None,
            source: "".to_string(),
            source_url: "".to_string(),
            actual: None,
            previous: None,
            forecast: None,
            actual_raw: None,
            previous_raw: None,
            forecast_raw: None,
            currency: "USD".to_string(),
            unit: None,
            scale: None,
            importance,
            date: Utc::now(),
        };

        let now = Utc::now();
        let req = EconomicCalendarRequest::builder()
            .from(now)
            .to(now)
            .min_importance(EconomicImportance::Medium)
            .build();

        assert!(!req.matches_filter(&make_event(-1)));
        assert!(req.matches_filter(&make_event(0)));
        assert!(req.matches_filter(&make_event(1)));
    }

    #[tokio::test]
    async fn test_request_validation_fails_locally() {
        let from = Utc.with_ymd_and_hms(2025, 1, 5, 0, 0, 0).unwrap();
        let to = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();

        let req_invalid_dates = EconomicCalendarRequest::new(from, to);
        assert!(get_economic_calendar(&req_invalid_dates).await.is_err());

        let req_invalid_country = EconomicCalendarRequest::builder()
            .from(to)
            .to(from)
            .countries(vec!["INVALID".to_string()])
            .build();
        assert!(get_economic_calendar(&req_invalid_country).await.is_err());
    }

    #[tokio::test]
    #[ignore = "requires network access to economic-calendar.tradingview.com"]
    async fn test_live_fetch() -> Result<()> {
        let from = Utc::now() - chrono::Duration::days(2);
        let to = Utc::now() + chrono::Duration::days(5);
        let req = EconomicCalendarRequest::builder()
            .from(from)
            .to(to)
            .countries(vec!["US".to_string(), "DE".to_string()])
            .min_importance(EconomicImportance::Medium)
            .build();

        let events = get_economic_calendar(&req).await?;
        assert!(!events.is_empty());
        for ev in &events {
            assert!(ev.importance >= 0);
        }
        Ok(())
    }
}
