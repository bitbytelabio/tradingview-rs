use chrono::NaiveDate;
use rand::Rng;
use std::{env::temp_dir, path::PathBuf};
use ustr::Ustr;

use crate::{
    chart::StudyOptions,
    fundamental::{
        entry::FundamentalRegistryEntry,
        filter::FundamentalRegistryFilter,
        registry::{FundamentalRegistry, extract_base_metric},
    },
    models::{
        FinancialPeriod,
        pine_indicator::{PineInfo, PineInfoExtra, ScriptType},
    },
};

fn sample_pine_info(
    script_name: &str,
    script_id: &str,
    script_version: &str,
    fund_id: Option<&str>,
    period: Option<FinancialPeriod>,
    is_fundamental: bool,
    category: Option<&str>,
) -> PineInfo {
    PineInfo {
        user_id: 100,
        script_name: Ustr::from(script_name),
        script_source: Ustr::from("study()"),
        script_id: Ustr::from(script_id),
        script_access: Ustr::from("public"),
        script_version: Ustr::from(script_version),
        extra: PineInfoExtra {
            financial_period: period,
            fund_id: fund_id.map(Ustr::from),
            fundamental_category: category.map(Ustr::from),
            is_fundamental_study: is_fundamental,
            short_description: Ustr::from(&format!("Description for {script_name}")),
            ..Default::default()
        },
    }
}

fn sample_entry(
    fund_id: &str,
    script_id: &str,
    version: &str,
    name: &str,
    period: Option<FinancialPeriod>,
    category: Option<&str>,
) -> FundamentalRegistryEntry {
    FundamentalRegistryEntry {
        fund_id: Ustr::from(fund_id),
        script_id: Ustr::from(script_id),
        script_version: Ustr::from(version),
        script_name: Ustr::from(name),
        financial_period: period,
        fundamental_category: category.map(Ustr::from),
        short_description: Some(Ustr::from("A fundamental test study")),
    }
}

#[test]
fn test_reject_non_fundamental_study() {
    let non_fund = sample_pine_info(
        "RSI",
        "STD;RSI",
        "1.0",
        Some("rsi"),
        None,
        false, // not fundamental
        None,
    );

    assert!(FundamentalRegistryEntry::from_pine_info(non_fund.clone()).is_none());
    assert!(FundamentalRegistryEntry::try_from(non_fund.clone()).is_err());

    let date = NaiveDate::from_ymd_opt(2026, 9, 9).unwrap();
    let registry = FundamentalRegistry::from_pine_infos(date, vec![non_fund]);
    assert_eq!(registry.len(), 0);
}

#[test]
fn test_reject_missing_fund_id() {
    let missing_fund_id = sample_pine_info(
        "Broken Metric",
        "STD;Broken",
        "1.0",
        None, // missing fund_id
        Some(FinancialPeriod::FiscalYear),
        true,
        Some("income_statement"),
    );

    assert!(FundamentalRegistryEntry::from_pine_info(missing_fund_id.clone()).is_none());
    assert!(FundamentalRegistryEntry::try_from(missing_fund_id.clone()).is_err());

    let date = NaiveDate::from_ymd_opt(2026, 9, 9).unwrap();
    let registry = FundamentalRegistry::from_pine_infos(date, vec![missing_fund_id]);
    assert_eq!(registry.len(), 0);
}

#[test]
fn test_deterministic_ordering() {
    let e1 = sample_entry(
        "total_revenue_fy",
        "STD;Total_Revenue_FY",
        "1.0",
        "Total Revenue",
        Some(FinancialPeriod::FiscalYear),
        Some("income_statement"),
    );
    let e2 = sample_entry(
        "total_revenue_fq",
        "STD;Total_Revenue_FQ",
        "1.0",
        "Total Revenue",
        Some(FinancialPeriod::FiscalQuarter),
        Some("income_statement"),
    );
    let e3 = sample_entry(
        "total_revenue_ttm",
        "STD;Total_Revenue_TTM",
        "2.0",
        "Total Revenue",
        Some(FinancialPeriod::TrailingTwelveMonths),
        Some("income_statement"),
    );
    let e4 = sample_entry(
        "balance_sheet_cash_fy",
        "STD;Cash_FY",
        "1.0",
        "Cash & Equivalents",
        Some(FinancialPeriod::FiscalYear),
        Some("balance_sheet"),
    );
    let e5 = sample_entry(
        "shares_outstanding_noagg",
        "STD;Shares_NOAGG",
        "1.0",
        "Shares Outstanding",
        Some(FinancialPeriod::UnknownPeriod("NOAGG".to_string())),
        Some("balance_sheet"),
    );

    let date = NaiveDate::from_ymd_opt(2026, 9, 9).unwrap();

    // Pass in reverse order
    let reg1 = FundamentalRegistry::new(
        date,
        vec![e1.clone(), e2.clone(), e3.clone(), e4.clone(), e5.clone()],
    );
    let reg2 = FundamentalRegistry::new(
        date,
        vec![e5.clone(), e4.clone(), e3.clone(), e2.clone(), e1.clone()],
    );

    assert_eq!(reg1.entries(), reg2.entries());

    // Deterministic sort ordering check:
    // "balance_sheet_cash_fy" < "shares_outstanding_noagg" < "total_revenue_fq" < "total_revenue_fy" < "total_revenue_ttm"
    assert_eq!(reg1.entries()[0].fund_id.as_str(), "balance_sheet_cash_fy");
    assert_eq!(
        reg1.entries()[1].fund_id.as_str(),
        "shares_outstanding_noagg"
    );
    assert_eq!(reg1.entries()[2].fund_id.as_str(), "total_revenue_fq");
    assert_eq!(reg1.entries()[3].fund_id.as_str(), "total_revenue_fy");
    assert_eq!(reg1.entries()[4].fund_id.as_str(), "total_revenue_ttm");
}

#[test]
fn test_round_trip_json_persistence() {
    let date = NaiveDate::from_ymd_opt(2026, 9, 9).unwrap();
    let entries = vec![
        sample_entry(
            "total_revenue_fy",
            "STD;Total_Revenue_FY",
            "1.0",
            "Total Revenue",
            Some(FinancialPeriod::FiscalYear),
            Some("income_statement"),
        ),
        sample_entry(
            "net_income_fq",
            "STD;Net_Income_FQ",
            "1.0",
            "Net Income",
            Some(FinancialPeriod::FiscalQuarter),
            Some("income_statement"),
        ),
        sample_entry(
            "shares_noagg",
            "STD;Shares_NOAGG",
            "1.0",
            "Shares Outstanding",
            Some(FinancialPeriod::UnknownPeriod("NOAGG".to_string())),
            Some("balance_sheet"),
        ),
    ];

    let original = FundamentalRegistry::new(date, entries);
    let json = original
        .to_json_pretty()
        .expect("serialization should succeed");

    assert!(json.contains("\"date\": \"2026-09-09\""));
    assert!(json.contains("\"version\": 1"));
    assert!(json.contains("\"financial_period\": \"FY\""));
    assert!(json.contains("\"financial_period\": \"FQ\""));
    assert!(json.contains("\"financial_period\": \"NOAGG\""));

    let loaded = FundamentalRegistry::from_json(&json).expect("deserialization should succeed");
    assert_eq!(original.date, loaded.date);
    assert_eq!(original.version, loaded.version);
    assert_eq!(original.entries, loaded.entries);

    // Ensure index was reconstructed upon deserialization
    assert!(loaded.get("total_revenue_fy").is_some());
    assert!(
        loaded
            .lookup("total_revenue", Some(&FinancialPeriod::FiscalYear))
            .is_some()
    );
    assert!(loaded.get("shares_noagg").is_some());
}

#[test]
fn test_lookup_and_period_variants() {
    let date = NaiveDate::from_ymd_opt(2026, 9, 9).unwrap();
    let entries = vec![
        sample_entry(
            "total_revenue_fy",
            "STD;Total_Revenue_FY",
            "1.0",
            "Total Revenue",
            Some(FinancialPeriod::FiscalYear),
            Some("income_statement"),
        ),
        sample_entry(
            "total_revenue_fq",
            "STD;Total_Revenue_FQ",
            "1.0",
            "Total Revenue",
            Some(FinancialPeriod::FiscalQuarter),
            Some("income_statement"),
        ),
        sample_entry(
            "total_revenue_ttm",
            "STD;Total_Revenue_TTM",
            "1.0",
            "Total Revenue",
            Some(FinancialPeriod::TrailingTwelveMonths),
            Some("income_statement"),
        ),
        sample_entry(
            "ebitda_noagg",
            "STD;EBITDA_NOAGG",
            "1.0",
            "EBITDA",
            Some(FinancialPeriod::UnknownPeriod("NOAGG".to_string())),
            Some("income_statement"),
        ),
    ];

    let reg = FundamentalRegistry::new(date, entries);

    // Exact match
    let fy_exact = reg.get("total_revenue_fy");
    assert!(fy_exact.is_some());
    assert_eq!(
        fy_exact.unwrap().financial_period,
        Some(FinancialPeriod::FiscalYear)
    );

    // Base metric + Period convenience
    let fy = reg.lookup("total_revenue", Some(&FinancialPeriod::FiscalYear));
    assert!(fy.is_some());
    assert_eq!(fy.unwrap().script_id.as_str(), "STD;Total_Revenue_FY");

    let fq = reg.lookup("total_revenue", Some(&FinancialPeriod::FiscalQuarter));
    assert!(fq.is_some());
    assert_eq!(fq.unwrap().script_id.as_str(), "STD;Total_Revenue_FQ");

    let ttm = reg.get_with_period("total_revenue", &FinancialPeriod::TrailingTwelveMonths);
    assert!(ttm.is_some());
    assert_eq!(ttm.unwrap().script_id.as_str(), "STD;Total_Revenue_TTM");

    // Custom period lookup
    let noagg = reg.lookup(
        "ebitda",
        Some(&FinancialPeriod::UnknownPeriod("NOAGG".to_string())),
    );
    assert!(noagg.is_some());
    assert_eq!(noagg.unwrap().script_id.as_str(), "STD;EBITDA_NOAGG");

    // Default variant when period is None (prefers FY)
    let default_rev = reg.lookup("total_revenue", None);
    assert!(default_rev.is_some());
    assert_eq!(
        default_rev.unwrap().financial_period,
        Some(FinancialPeriod::FiscalYear)
    );

    // Nonexistent lookup should return None safely
    assert!(reg.get("nonexistent_metric").is_none());
    assert!(reg.lookup("nonexistent_metric", None).is_none());
    assert!(
        reg.lookup("total_revenue", Some(&FinancialPeriod::FiscalHalfYear))
            .is_none()
    );

    // Variants query
    let variants = reg.get_variants("total_revenue");
    assert_eq!(variants.len(), 3);
}

#[test]
fn test_filter_and_search() {
    let date = NaiveDate::from_ymd_opt(2026, 9, 9).unwrap();
    let entries = vec![
        sample_entry(
            "total_revenue_fy",
            "STD;Total_Revenue_FY",
            "1.0",
            "Total Revenue",
            Some(FinancialPeriod::FiscalYear),
            Some("income_statement"),
        ),
        sample_entry(
            "net_income_fy",
            "STD;Net_Income_FY",
            "1.0",
            "Net Income",
            Some(FinancialPeriod::FiscalYear),
            Some("income_statement"),
        ),
        sample_entry(
            "total_assets_fy",
            "STD;Total_Assets_FY",
            "1.0",
            "Total Assets",
            Some(FinancialPeriod::FiscalYear),
            Some("balance_sheet"),
        ),
    ];

    let reg = FundamentalRegistry::new(date, entries);

    // Category search
    let bs = reg.find_by_category("balance_sheet");
    assert_eq!(bs.len(), 1);
    assert_eq!(bs[0].fund_id.as_str(), "total_assets_fy");

    let is = reg.find_by_category("income_statement");
    assert_eq!(is.len(), 2);

    // Text search
    let search_res = reg.search("income");
    assert_eq!(search_res.len(), 2); // total_revenue_fy (by category) and net_income_fy (by category and name)

    // Filter builder
    let filter = FundamentalRegistryFilter::builder()
        .category("income_statement".to_string())
        .name("Revenue".to_string())
        .build();
    let filtered = reg.filter(&filter);
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].fund_id.as_str(), "total_revenue_fy");
}

#[test]
fn test_to_study_options() {
    let entry = sample_entry(
        "total_revenue_fy",
        "STD;Total_Revenue_FY",
        "1.0",
        "Total Revenue",
        Some(FinancialPeriod::FiscalYear),
        Some("income_statement"),
    );

    let opts: StudyOptions = entry.to_study_options();
    assert_eq!(opts.script_id.as_str(), "STD;Total_Revenue_FY");
    assert_eq!(opts.script_version.as_str(), "1.0");
    assert_eq!(opts.script_type, ScriptType::IntervalScript);
}

#[test]
fn test_base_metric_extraction() {
    assert_eq!(
        extract_base_metric("total_revenue_fy", Some(&FinancialPeriod::FiscalYear)),
        "total_revenue"
    );
    assert_eq!(
        extract_base_metric("net_income_fq", Some(&FinancialPeriod::FiscalQuarter)),
        "net_income"
    );
    assert_eq!(
        extract_base_metric(
            "free_cash_flow_ttm",
            Some(&FinancialPeriod::TrailingTwelveMonths)
        ),
        "free_cash_flow"
    );
    assert_eq!(
        extract_base_metric("shares_outstanding_noagg", None),
        "shares_outstanding"
    );
    assert_eq!(extract_base_metric("custom_metric", None), "custom_metric");
}

fn temp_test_path() -> PathBuf {
    let mut rng = rand::rng();
    let num: u64 = rng.random();
    temp_dir().join(format!("tv_fund_test_{num}.json"))
}

#[tokio::test]
async fn test_file_persistence_sync_and_async() {
    let date = NaiveDate::from_ymd_opt(2026, 9, 9).unwrap();
    let entries = vec![sample_entry(
        "total_revenue_fy",
        "STD;Total_Revenue_FY",
        "1.0",
        "Total Revenue",
        Some(FinancialPeriod::FiscalYear),
        Some("income_statement"),
    )];

    let registry = FundamentalRegistry::new(date, entries);

    // Sync save/load
    let sync_path = temp_test_path();
    registry
        .save_to_file(&sync_path)
        .expect("sync save should succeed");
    let loaded_sync =
        FundamentalRegistry::load_from_file(&sync_path).expect("sync load should succeed");
    assert_eq!(registry.entries, loaded_sync.entries);
    let _ = std::fs::remove_file(&sync_path);

    // Async save/load
    let async_path = temp_test_path();
    registry
        .save_to_file_async(&async_path)
        .await
        .expect("async save should succeed");
    let loaded_async = FundamentalRegistry::load_from_file_async(&async_path)
        .await
        .expect("async load should succeed");
    assert_eq!(registry.entries, loaded_async.entries);
    let _ = std::fs::remove_file(&async_path);
}

#[test]
fn test_version_tag_and_iter_variants() {
    let date = NaiveDate::from_ymd_opt(2026, 9, 9).unwrap();
    let entries = vec![
        sample_entry(
            "total_revenue_fy",
            "STD;Total_Revenue_FY",
            "1.0",
            "Total Revenue",
            Some(FinancialPeriod::FiscalYear),
            Some("income_statement"),
        ),
        sample_entry(
            "total_revenue_fq",
            "STD;Total_Revenue_FQ",
            "1.0",
            "Total Revenue",
            Some(FinancialPeriod::FiscalQuarter),
            Some("income_statement"),
        ),
    ];

    let registry = FundamentalRegistry::new(date, entries);
    assert_eq!(registry.version_tag(), "fundamentals-2026-09-09");
    assert_eq!(registry.version(), 1);

    let count = registry.iter_variants("total_revenue").count();
    assert_eq!(count, 2);
}

#[test]
fn test_snake_case_fund_id_wire_support() {
    // Raw JSON resembling TradingView pine-facade response with snake_case "fund_id"
    let json = r#"[
        {
            "userId": 0,
            "scriptName": "Total Revenue",
            "scriptSource": "",
            "scriptIdPart": "STD;Total_Revenue_FY",
            "version": "1.0",
            "extra": {
                "isFundamentalStudy": true,
                "fund_id": "total_revenue_fy",
                "financialPeriod": "FY",
                "fundamentalCategory": "income_statement",
                "shortDescription": "Total Revenue"
            }
        }
    ]"#;

    let raw_items: Vec<crate::fundamental::fetch::RawPineFacadeItem> =
        serde_json::from_str(json).unwrap();
    let pine_infos: Vec<PineInfo> = raw_items.into_iter().map(PineInfo::from).collect();
    let date = NaiveDate::from_ymd_opt(2026, 9, 9).unwrap();
    let registry = FundamentalRegistry::from_pine_infos(date, pine_infos);

    assert_eq!(registry.len(), 1);
    let entry = registry.get("total_revenue_fy");
    assert!(entry.is_some());
    assert_eq!(entry.unwrap().fund_id.as_str(), "total_revenue_fy");
}
