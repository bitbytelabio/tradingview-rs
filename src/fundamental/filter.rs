//! Search and filtering criteria for the fundamental Pine study registry.

use bon::Builder;

use crate::{fundamental::entry::FundamentalRegistryEntry, models::FinancialPeriod};

/// Checks whether `haystack` contains `needle` ignoring ASCII case.
///
/// Operates directly on byte slices with zero heap allocations.
#[inline]
pub fn contains_ignore_ascii_case(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    let n = needle.len();
    if n > haystack.len() {
        return false;
    }
    let needle_bytes = needle.as_bytes();
    let haystack_bytes = haystack.as_bytes();
    haystack_bytes
        .windows(n)
        .any(|window| window.eq_ignore_ascii_case(needle_bytes))
}

/// Filter criteria for querying fundamental studies in [`super::FundamentalRegistry`].
///
/// All fields are optional and combine with logical AND semantics.
/// String matching is ASCII-case-insensitive substring matching without heap allocation.
#[derive(Debug, Default, Clone, PartialEq, Builder)]
pub struct FundamentalRegistryFilter {
    /// Category filter (e.g. `"income_statement"`, `"balance_sheet"`).
    pub category: Option<String>,
    /// Study name filter (e.g. `"Revenue"`, `"Cash Flow"`).
    pub name: Option<String>,
    /// Fund ID filter (e.g. `"revenue"`, `"ebitda"`).
    pub fund_id: Option<String>,
    /// Exact financial reporting period filter.
    pub period: Option<FinancialPeriod>,
    /// General text search across study name, fund ID, category, and description.
    pub query: Option<String>,
}

impl FundamentalRegistryFilter {
    /// Returns `true` if `entry` matches all active filter criteria.
    ///
    /// Performs ASCII-case-insensitive matching without per-entry allocations.
    pub fn matches(&self, entry: &FundamentalRegistryEntry) -> bool {
        if let Some(cat) = &self.category {
            match &entry.fundamental_category {
                Some(entry_cat) => {
                    if !contains_ignore_ascii_case(entry_cat.as_str(), cat) {
                        return false;
                    }
                }
                None => return false,
            }
        }

        if let Some(name) = &self.name
            && !contains_ignore_ascii_case(entry.script_name.as_str(), name)
        {
            return false;
        }

        if let Some(fid) = &self.fund_id
            && !contains_ignore_ascii_case(entry.fund_id.as_str(), fid)
        {
            return false;
        }

        if let Some(req_period) = &self.period
            && entry.financial_period.as_ref() != Some(req_period)
        {
            return false;
        }

        if let Some(q) = &self.query {
            let matches_name = contains_ignore_ascii_case(entry.script_name.as_str(), q);
            let matches_fund_id = contains_ignore_ascii_case(entry.fund_id.as_str(), q);
            let matches_cat = entry
                .fundamental_category
                .map(|c| contains_ignore_ascii_case(c.as_str(), q))
                .unwrap_or(false);
            let matches_desc = entry
                .short_description
                .map(|d| contains_ignore_ascii_case(d.as_str(), q))
                .unwrap_or(false);

            if !matches_name && !matches_fund_id && !matches_cat && !matches_desc {
                return false;
            }
        }

        true
    }
}
