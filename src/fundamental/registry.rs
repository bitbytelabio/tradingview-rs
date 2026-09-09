//! Fundamental Pine study registry with deterministic sorting, fast lookup,
//! and persistence.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::{
    Result,
    fundamental::{entry::FundamentalRegistryEntry, filter::FundamentalRegistryFilter},
    models::{FinancialPeriod, pine_indicator::PineInfo},
};

/// Strips period suffixes from a `fund_id` to locate its base metric name.
pub(crate) fn extract_base_metric<'a>(
    fund_id: &'a str,
    period: Option<&FinancialPeriod>,
) -> &'a str {
    if let Some(p) = period {
        let suffix = format!("_{}", p.to_string().to_lowercase());
        if let Some(stripped) = fund_id.strip_suffix(&suffix) {
            return stripped;
        }
    }
    for suffix in &[
        "_fy", "_fq", "_fh", "_ttm", "_noagg", "_nfq", "_nfy", "_nfh", "_n4fy", "_n4fq", "_n4fh",
        "_ntm", "_agg",
    ] {
        if let Some(stripped) = fund_id.strip_suffix(suffix) {
            return stripped;
        }
    }
    fund_id
}

/// Date-versioned registry of TradingView fundamental Pine studies.
///
/// Holds an ordered, deduplicated catalog of fundamental studies tagged with
/// a UTC snapshot date. Exposes fast non-panicking lookup by exact or base `fund_id`,
/// filtering, search, and JSON persistence.
///
/// Lookups operate directly over the deterministically sorted array using binary search
/// and partition points, avoiding hash map allocations and never interning external
/// lookup strings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(from = "RawFundamentalRegistry")]
pub struct FundamentalRegistry {
    /// UTC date when this registry snapshot was captured.
    pub date: NaiveDate,
    /// Schema / registry format version.
    pub version: u32,
    /// Deterministically sorted fundamental entries.
    pub entries: Vec<FundamentalRegistryEntry>,
}

#[derive(Deserialize)]
struct RawFundamentalRegistry {
    date: NaiveDate,
    #[serde(default = "default_registry_version")]
    version: u32,
    entries: Vec<FundamentalRegistryEntry>,
}

fn default_registry_version() -> u32 {
    1
}

impl From<RawFundamentalRegistry> for FundamentalRegistry {
    fn from(raw: RawFundamentalRegistry) -> Self {
        let mut reg = Self::new(raw.date, raw.entries);
        reg.version = raw.version;
        reg
    }
}

impl Default for FundamentalRegistry {
    fn default() -> Self {
        Self::new(NaiveDate::from_ymd_opt(1970, 1, 1).unwrap(), Vec::new())
    }
}

impl FundamentalRegistry {
    /// Creates a new [`FundamentalRegistry`] from a UTC date and a list of entries.
    ///
    /// Entries are deterministically sorted by `(fund_id, financial_period, script_id, script_version)`.
    pub fn new(date: NaiveDate, mut entries: Vec<FundamentalRegistryEntry>) -> Self {
        entries.sort();
        entries.dedup();
        Self {
            date,
            version: 1,
            entries,
        }
    }

    /// Constructs a registry from raw [`PineInfo`] indicators returned by TradingView's
    /// Pine facade, keeping only entries where `is_fundamental_study == true` and `fund_id` is present.
    pub fn from_pine_infos(date: NaiveDate, infos: impl IntoIterator<Item = PineInfo>) -> Self {
        let entries: Vec<FundamentalRegistryEntry> = infos
            .into_iter()
            .filter(|info| info.extra.is_fundamental_study)
            .filter_map(FundamentalRegistryEntry::from_pine_info)
            .collect();
        Self::new(date, entries)
    }

    /// Number of fundamental studies registered.
    #[inline]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if the registry contains no entries.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// UTC date of this registry snapshot.
    #[inline]
    pub fn date(&self) -> NaiveDate {
        self.date
    }

    /// Schema / serialization format version of this registry structure (e.g. `1`).
    #[inline]
    pub fn version(&self) -> u32 {
        self.version
    }

    /// Stable date-versioned tag identifier for this registry snapshot (e.g. `"fundamentals-2026-09-09"`).
    #[inline]
    pub fn version_tag(&self) -> String {
        format!("fundamentals-{}", self.date)
    }

    /// Slice of all registered entries in deterministic order.
    #[inline]
    pub fn entries(&self) -> &[FundamentalRegistryEntry] {
        &self.entries
    }

    /// Iterator over references to all registered entries.
    #[inline]
    pub fn iter(&self) -> std::slice::Iter<'_, FundamentalRegistryEntry> {
        self.entries.iter()
    }

    /// Fast lookup by exact `fund_id` (e.g. `"total_revenue_fy"`) using binary search.
    ///
    /// Borrows the input string slice and avoids interning arbitrary lookups into global memory.
    /// Returns `None` if no entry with this exact `fund_id` exists.
    pub fn get(&self, fund_id: &str) -> Option<&FundamentalRegistryEntry> {
        let idx = self
            .entries
            .binary_search_by(|e| e.fund_id.as_str().cmp(fund_id))
            .ok()?;
        self.entries.get(idx)
    }

    /// Non-panicking lookup by `fund_id` (either full or base) and optional [`FinancialPeriod`].
    ///
    /// # Lookup Resolution Order
    /// 1. If `period` is `None`:
    ///    - Checks exact `fund_id`.
    ///    - If not found, checks if `fund_id` is a base metric and returns its preferred variant
    ///      (favoring FY, then TTM, then FQ, or first available).
    /// 2. If `period` is `Some(p)`:
    ///    - If exact `fund_id` exists and its period matches `p`, returns it.
    ///    - Checks synthesized identifier `{fund_id}_{p}`.
    ///    - Resolves base metric and checks its period variants.
    pub fn lookup(
        &self,
        fund_id: &str,
        period: Option<&FinancialPeriod>,
    ) -> Option<&FundamentalRegistryEntry> {
        match period {
            None => {
                // 1. Exact match
                if let Some(entry) = self.get(fund_id) {
                    return Some(entry);
                }
                // 2. Base metric lookup: iterate contiguous prefix variants directly
                let mut first = None;
                let mut ttm = None;
                for v in self.iter_variants(fund_id) {
                    if let Some(FinancialPeriod::FiscalYear) = v.financial_period {
                        return Some(v);
                    }
                    if let Some(FinancialPeriod::TrailingTwelveMonths) = v.financial_period {
                        ttm = Some(v);
                    }
                    if first.is_none() {
                        first = Some(v);
                    }
                }
                ttm.or(first)
            }
            Some(target_period) => {
                if let Some(entry) = self.get(fund_id)
                    && entry.financial_period.as_ref() == Some(target_period)
                {
                    return Some(entry);
                }
                // 2. Try synthesized suffix, e.g. {fund_id}_{period}
                let p_str = target_period.to_string().to_lowercase();
                let candidate_id = format!("{fund_id}_{p_str}");
                if let Some(entry) = self.get(&candidate_id) {
                    return Some(entry);
                }

                // 3. Extract base metric from fund_id and look up with candidate
                let base_str = extract_base_metric(fund_id, Some(target_period));
                if base_str != fund_id {
                    let base_candidate = format!("{base_str}_{p_str}");
                    if let Some(entry) = self.get(&base_candidate) {
                        return Some(entry);
                    }
                }

                // 4. Scan base metric variants directly without Vec allocation
                for v in self.iter_variants(base_str) {
                    if v.financial_period.as_ref() == Some(target_period) {
                        return Some(v);
                    }
                }

                None
            }
        }
    }

    /// Convenience lookup for a base metric and specific [`FinancialPeriod`].
    #[inline]
    pub fn get_with_period(
        &self,
        base_metric: &str,
        period: &FinancialPeriod,
    ) -> Option<&FundamentalRegistryEntry> {
        self.lookup(base_metric, Some(period))
    }

    /// Returns an iterator over all reporting period variants for a given base metric without heap allocation.
    pub fn iter_variants<'a>(
        &'a self,
        metric: &str,
    ) -> impl Iterator<Item = &'a FundamentalRegistryEntry> {
        let base_str = extract_base_metric(metric, None);
        let prefix = format!("{base_str}_");
        let start = self
            .entries
            .partition_point(|e| e.fund_id.as_str() < prefix.as_str());

        let count = self.entries[start..]
            .iter()
            .take_while(|e| e.fund_id.as_str().starts_with(&prefix))
            .count();

        let prefix_slice = &self.entries[start..start + count];
        let exact = self
            .get(base_str)
            .filter(|e| !prefix_slice.iter().any(|p| p.fund_id == e.fund_id));

        prefix_slice.iter().chain(exact)
    }

    /// Returns all available reporting period variants for a given base metric.
    #[inline]
    pub fn get_variants(&self, metric: &str) -> Vec<&FundamentalRegistryEntry> {
        self.iter_variants(metric).collect()
    }

    /// Filters entries using a [`FundamentalRegistryFilter`].
    pub fn filter(&self, filter: &FundamentalRegistryFilter) -> Vec<&FundamentalRegistryEntry> {
        self.entries.iter().filter(|e| filter.matches(e)).collect()
    }

    /// Searches entries by text query across name, fund ID, category, and description.
    pub fn search(&self, query: &str) -> Vec<&FundamentalRegistryEntry> {
        let filter = FundamentalRegistryFilter::builder()
            .query(query.to_string())
            .build();
        self.filter(&filter)
    }

    /// Finds all entries matching the specified fundamental accounting category.
    pub fn find_by_category(&self, category: &str) -> Vec<&FundamentalRegistryEntry> {
        let filter = FundamentalRegistryFilter::builder()
            .category(category.to_string())
            .build();
        self.filter(&filter)
    }

    /// Serializes the registry to a compact JSON string.
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string(self).map_err(|e| crate::Error::JsonParse(e.to_string().into()))
    }

    /// Serializes the registry to a pretty-printed JSON string.
    pub fn to_json_pretty(&self) -> Result<String> {
        serde_json::to_string_pretty(self)
            .map_err(|e| crate::Error::JsonParse(e.to_string().into()))
    }

    /// Deserializes a registry from a JSON string.
    pub fn from_json(json: &str) -> Result<Self> {
        serde_json::from_str(json).map_err(|e| crate::Error::JsonParse(e.to_string().into()))
    }

    /// Saves the registry to a file at the specified path (creating or overwriting).
    ///
    /// Parent directories are created automatically if they do not exist.
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path_ref = path.as_ref();
        if let Some(parent) = path_ref.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }
        let json = self.to_json_pretty()?;
        std::fs::write(path_ref, json)?;
        Ok(())
    }

    /// Loads the registry from a file at the specified path.
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path)?;
        Self::from_json(&content)
    }

    /// Asynchronously saves the registry to a file at the specified path.
    ///
    /// Pre-serializes to JSON before offloading to a blocking write thread,
    /// avoiding full registry cloning across thread boundaries.
    pub async fn save_to_file_async<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let json = self.to_json_pretty()?;
        let path_buf = path.as_ref().to_path_buf();
        tokio::task::spawn_blocking(move || {
            if let Some(parent) = path_buf.parent()
                && !parent.as_os_str().is_empty()
            {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&path_buf, json)?;
            Ok(())
        })
        .await
        .map_err(|e| crate::Error::TokioJoin(e.to_string().into()))?
    }

    /// Asynchronously loads the registry from a file at the specified path.
    pub async fn load_from_file_async<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path_buf = path.as_ref().to_path_buf();
        tokio::task::spawn_blocking(move || Self::load_from_file(path_buf))
            .await
            .map_err(|e| crate::Error::TokioJoin(e.to_string().into()))?
    }
}

impl<'a> IntoIterator for &'a FundamentalRegistry {
    type Item = &'a FundamentalRegistryEntry;
    type IntoIter = std::slice::Iter<'a, FundamentalRegistryEntry>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter()
    }
}

impl IntoIterator for FundamentalRegistry {
    type Item = FundamentalRegistryEntry;
    type IntoIter = std::vec::IntoIter<FundamentalRegistryEntry>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.into_iter()
    }
}
