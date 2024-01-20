use serde::Deserialize;
use serde::Serialize;

pub enum NewsArea {
    World,
    Americas,
    Europe,
    Asia,
    Oceania,
    Africa,
}

pub enum NewsSection {
    AnalysisAll,
    AnalysisRecommendations,
    EstimatesAndForecasts,
    MarketToday,
    Surveys,
    PressRelease,
    FinancialStatement,
    InsiderTrading,
    ESG,
    CorpActivitiesAll,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NewsHeadlines {
    #[serde(rename = "items")]
    pub items: Vec<News>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct News {
    pub id: String,
    pub title: String,
    pub provider: String,
    pub published: i64,
    pub source: String,
    pub urgency: i64,
    pub permission: Option<String>,
    #[serde(default)]
    pub related_symbols: Vec<RelatedSymbol>,
    pub story_path: String,
    pub link: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RelatedSymbol {
    pub symbol: String,
}
