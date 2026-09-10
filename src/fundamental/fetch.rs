//! Fetching fundamental Pine studies from TradingView.

use chrono::NaiveDate;
use reqwest::header::USER_AGENT;
use serde::Deserialize;
use ustr::Ustr;

use crate::{
    Result, UA,
    fundamental::registry::FundamentalRegistry,
    models::{
        FinancialPeriod,
        pine_indicator::{PineInfo, PineInfoExtra},
    },
    utils::http_client,
};

/// TradingView pine-facade endpoint for fundamental indicators.
pub const PINE_FACADE_FUNDAMENTAL_URL: &str =
    "https://pine-facade.tradingview.com/pine-facade/list/?filter=fundamental";

/// Wire model matching TradingView's `/pine-facade/list` response.
///
/// Accepts both snake_case `"fund_id"` (returned by TradingView's API)
/// and camelCase `"fundId"` for robustness against upstream serialization differences.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RawPineFacadeItem {
    user_id: Option<i64>,
    script_name: Ustr,
    #[serde(default)]
    script_source: Ustr,
    #[serde(rename = "scriptIdPart")]
    script_id: Ustr,
    #[serde(default)]
    script_access: Ustr,
    version: Ustr,
    #[serde(default)]
    extra: RawPineFacadeExtra,
}

#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct RawPineFacadeExtra {
    financial_period: Option<FinancialPeriod>,
    #[serde(alias = "fund_id", alias = "fundId")]
    fund_id: Option<Ustr>,
    fundamental_category: Option<Ustr>,
    is_fundamental_study: bool,
    short_description: Ustr,
}

impl From<RawPineFacadeItem> for PineInfo {
    fn from(item: RawPineFacadeItem) -> Self {
        PineInfo {
            user_id: item.user_id.unwrap_or_default(),
            script_name: item.script_name,
            script_source: item.script_source,
            script_id: item.script_id,
            script_access: item.script_access,
            script_version: item.version,
            extra: PineInfoExtra {
                financial_period: item.extra.financial_period,
                fund_id: item.extra.fund_id,
                fundamental_category: item.extra.fundamental_category,
                is_fundamental_study: item.extra.is_fundamental_study,
                short_description: item.extra.short_description,
                ..Default::default()
            },
        }
    }
}

/// Fetches the fundamental Pine study list from TradingView and builds a
/// date-versioned [`FundamentalRegistry`] snapshot tagged with the current UTC date.
///
/// Uses the crate's shared HTTP client for pooled connections.
pub async fn fetch_fundamental_registry() -> Result<FundamentalRegistry> {
    fetch_fundamental_registry_with_client(http_client()).await
}

/// Fetches the fundamental Pine study list using the provided HTTP client.
pub async fn fetch_fundamental_registry_with_client(
    client: reqwest::Client,
) -> Result<FundamentalRegistry> {
    let date = chrono::Utc::now().date_naive();
    fetch_fundamental_registry_for_date(client, date).await
}

/// Fetches the fundamental Pine study list for a specific UTC date tag.
pub async fn fetch_fundamental_registry_for_date(
    client: reqwest::Client,
    date: NaiveDate,
) -> Result<FundamentalRegistry> {
    let response = client
        .get(PINE_FACADE_FUNDAMENTAL_URL)
        .header(USER_AGENT, UA)
        .send()
        .await
        .map_err(|e| crate::Error::Request(e.to_string().into()))?;

    let status = response.status();
    if !status.is_success() {
        return Err(crate::Error::Request(
            format!("failed to fetch fundamental studies: HTTP {status}").into(),
        ));
    }

    let items = response
        .json::<Vec<RawPineFacadeItem>>()
        .await
        .map_err(|e| crate::Error::JsonParse(e.to_string().into()))?;

    let infos: Vec<PineInfo> = items.into_iter().map(PineInfo::from).collect();

    Ok(FundamentalRegistry::from_pine_infos(date, infos))
}
