//! Fundamental registry entry representation.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt::Display;
use ustr::Ustr;

use crate::{
    Result,
    chart::StudyOptions,
    models::{
        FinancialPeriod, UserCookies,
        pine_indicator::{PineIndicator, PineInfo, ScriptType},
    },
};

/// Custom serde module for `Option<FinancialPeriod>`.
///
/// TradingView represents financial reporting periods as strings
/// (`"FY"`, `"FQ"`, `"FH"`, `"TTM"`, `"NOAGG"`, etc.). Serializing directly
/// with derived untagged serde on unit variants produces `null`. This helper
/// ensures lossless string round-trip using [`Display`].
mod period_serde {
    use super::*;

    pub fn serialize<S>(
        period: &Option<FinancialPeriod>,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match period {
            Some(p) => serializer.serialize_some(&p.to_string()),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(
        deserializer: D,
    ) -> std::result::Result<Option<FinancialPeriod>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let opt: Option<String> = Option::deserialize(deserializer)?;
        Ok(opt.map(|s| match s.as_str() {
            "FY" => FinancialPeriod::FiscalYear,
            "FQ" => FinancialPeriod::FiscalQuarter,
            "FH" => FinancialPeriod::FiscalHalfYear,
            "TTM" => FinancialPeriod::TrailingTwelveMonths,
            _ => FinancialPeriod::UnknownPeriod(s),
        }))
    }
}

/// A single fundamental Pine study metric registered in TradingView's catalog.
///
/// Each fundamental indicator corresponds to a built-in Pine Script study
/// that computes a fundamental financial metric (e.g., Total Revenue, Net Income,
/// Debt to Equity) for a particular reporting period.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FundamentalRegistryEntry {
    /// Canonical TradingView metric identifier (e.g., `"total_revenue_fy"`, `"ebitda_ttm"`).
    pub fund_id: Ustr,
    /// Pine study identifier part (e.g., `"STD;Total_Revenue_FY"`).
    pub script_id: Ustr,
    /// Pine study version string (e.g., `"1.0"` or `"4"`).
    pub script_version: Ustr,
    /// Human-readable study title (e.g., `"Total Revenue"`).
    pub script_name: Ustr,
    /// Reporting period, if applicable (`FY`, `FQ`, `FH`, `TTM`, `NOAGG`, etc.).
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "period_serde"
    )]
    pub financial_period: Option<FinancialPeriod>,
    /// High-level fundamental accounting category (e.g., `"income_statement"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fundamental_category: Option<Ustr>,
    /// Brief textual description of the metric.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub short_description: Option<Ustr>,
}

fn cmp_period(a: Option<&FinancialPeriod>, b: Option<&FinancialPeriod>) -> std::cmp::Ordering {
    match (a, b) {
        (None, None) => std::cmp::Ordering::Equal,
        (None, Some(_)) => std::cmp::Ordering::Less,
        (Some(_), None) => std::cmp::Ordering::Greater,
        (Some(pa), Some(pb)) => pa.to_string().cmp(&pb.to_string()),
    }
}

impl Eq for FundamentalRegistryEntry {}

impl Ord for FundamentalRegistryEntry {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.fund_id
            .as_str()
            .cmp(other.fund_id.as_str())
            .then_with(|| {
                cmp_period(
                    self.financial_period.as_ref(),
                    other.financial_period.as_ref(),
                )
            })
            .then_with(|| self.script_id.as_str().cmp(other.script_id.as_str()))
            .then_with(|| {
                self.script_version
                    .as_str()
                    .cmp(other.script_version.as_str())
            })
    }
}

impl PartialOrd for FundamentalRegistryEntry {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Display for FundamentalRegistryEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.script_name, self.fund_id)?;
        if let Some(period) = &self.financial_period {
            write!(f, " [{period}]")?;
        }
        write!(f, " v{}", self.script_version)
    }
}

impl FundamentalRegistryEntry {
    /// Attempts to create a [`FundamentalRegistryEntry`] from a raw [`PineInfo`].
    ///
    /// Returns `None` if `info.extra.is_fundamental_study` is `false` or
    /// if `info.extra.fund_id` is missing.
    pub fn from_pine_info(info: PineInfo) -> Option<Self> {
        if !info.extra.is_fundamental_study {
            return None;
        }
        let fund_id = info.extra.fund_id?;
        Some(Self {
            fund_id,
            script_id: info.script_id,
            script_version: info.script_version,
            script_name: info.script_name,
            financial_period: info.extra.financial_period,
            fundamental_category: info.extra.fundamental_category,
            short_description: if info.extra.short_description.as_str().trim().is_empty() {
                None
            } else {
                Some(info.extra.short_description)
            },
        })
    }

    /// Returns the fundamental metric identifier as a string slice.
    #[inline]
    pub fn fund_id(&self) -> &str {
        self.fund_id.as_str()
    }

    /// Returns the Pine Script study identifier part as a string slice.
    #[inline]
    pub fn script_id(&self) -> &str {
        self.script_id.as_str()
    }

    /// Returns the Pine Script study version as a string slice.
    #[inline]
    pub fn script_version(&self) -> &str {
        self.script_version.as_str()
    }

    /// Returns the human-readable script name as a string slice.
    #[inline]
    pub fn script_name(&self) -> &str {
        self.script_name.as_str()
    }

    /// Returns the financial period, if present.
    #[inline]
    pub fn financial_period(&self) -> Option<&FinancialPeriod> {
        self.financial_period.as_ref()
    }

    /// Returns the accounting category, if present.
    #[inline]
    pub fn fundamental_category(&self) -> Option<&str> {
        self.fundamental_category.map(|c| c.as_str())
    }

    /// Returns the short description, if present.
    #[inline]
    pub fn short_description(&self) -> Option<&str> {
        self.short_description.map(|d| d.as_str())
    }

    /// Derives the base metric name by stripping known period suffixes from `fund_id`.
    ///
    /// For example:
    /// - `"total_revenue_fy"` -> `"total_revenue"`
    /// - `"net_income_fq"` -> `"net_income"`
    /// - `"free_cash_flow_ttm"` -> `"free_cash_flow"`
    /// - `"shares_outstanding_noagg"` -> `"shares_outstanding"`
    /// - `"pe_ratio"` -> `"pe_ratio"`
    pub fn base_metric(&self) -> &str {
        let fid = self.fund_id.as_str();
        if let Some(period) = &self.financial_period {
            let p_suffix = format!("_{}", period.to_string().to_lowercase());
            if let Some(stripped) = fid.strip_suffix(&p_suffix) {
                return stripped;
            }
        }
        // Fallback for suffixes without explicit period field:
        for suffix in &[
            "_fy", "_fq", "_fh", "_ttm", "_noagg", "_nfq", "_nfy", "_nfh", "_n4fy", "_n4fq",
            "_n4fh", "_ntm", "_agg",
        ] {
            if let Some(stripped) = fid.strip_suffix(suffix) {
                return stripped;
            }
        }
        fid
    }

    /// Converts this entry into a chart [`StudyOptions`] configuration.
    ///
    /// Note: Fundamental studies use [`ScriptType::IntervalScript`] on TradingView's
    /// WebSocket data feed.
    #[inline]
    pub fn to_study_options(&self) -> StudyOptions {
        StudyOptions {
            script_id: self.script_id,
            script_version: self.script_version,
            script_type: ScriptType::IntervalScript,
        }
    }

    /// Asynchronously fetches the full Pine indicator metadata, returning a [`PineIndicator`].
    ///
    /// The resulting [`PineIndicator`] can be directly passed into chart study configurations.
    pub async fn fetch_indicator(&self, user: Option<&UserCookies>) -> Result<PineIndicator> {
        let mut builder = PineIndicator::build();
        if let Some(cookies) = user {
            builder.user(cookies.clone());
        }
        builder
            .fetch(
                self.script_id.as_str(),
                self.script_version.as_str(),
                ScriptType::IntervalScript,
            )
            .await
    }
}

impl TryFrom<PineInfo> for FundamentalRegistryEntry {
    type Error = crate::Error;

    fn try_from(info: PineInfo) -> Result<Self> {
        Self::from_pine_info(info).ok_or_else(|| {
            crate::Error::Internal(
                "not a fundamental study (extra.is_fundamental_study is false or missing fund_id)"
                    .into(),
            )
        })
    }
}
