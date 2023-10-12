use std::collections::HashMap;

use tracing::info;

use super::mics::get_builtin_indicators;
use crate::{
    models::{
        pine_indicator::{ BuiltinIndicators, ScriptType, PineMetadataInfo },
        FinancialPeriod,
    },
    Result,
};

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum FundamentalCategory {
    BalanceSheet,
    CashFlow,
    IncomeStatement,
}

impl FundamentalCategory {
    pub fn from_str(s: &str) -> Option<Self> {
        // let s = s.to_lowercase().replace(" ", "");
        match s {
            "Balance sheet" => Some(Self::BalanceSheet),
            "Cash flow" => Some(Self::CashFlow),
            "Income statement" => Some(Self::IncomeStatement),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct FundamentalIndicator {
    pub name: String,
    pub period: FinancialPeriod,
    pub id: String,
    pub version: String,
    pub script_type: ScriptType,
    // pub metadata: PineMetadataInfo,
}

pub async fn test01() -> Result<()> {
    let indics = get_builtin_indicators(BuiltinIndicators::Fundamental).await?;

    let mut map: HashMap<FundamentalCategory, Vec<FundamentalIndicator>> = HashMap::new();
    indics
        .iter()
        .filter(|s| !s.extra.is_hidden_study && !s.extra.is_beta && s.extra.is_fundamental_study)
        .for_each(|i| {
            match &i.extra.fundamental_category {
                Some(category) => {
                    let category = FundamentalCategory::from_str(category);
                    match category {
                        Some(cat) => {
                            let vec = map.entry(cat).or_insert(vec![]);
                            vec.push(FundamentalIndicator {
                                name: i.script_name.clone(),
                                period: i.extra.financial_period.clone().unwrap(),
                                id: i.script_id.clone(),
                                version: i.script_version.clone(),
                                script_type: ScriptType::IntervalScript,
                                // metadata: i.extra,
                            });
                        }
                        None => {}
                    }
                }
                None => {}
            }
        });
    info!("{:#?}", map);
    Ok(())
}
