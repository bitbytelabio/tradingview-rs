use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

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
#[serde(default)]
pub struct NewsHeadlines {
    #[serde(rename = "items")]
    pub items: Vec<News>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
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

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct NewsContent {
    pub short_description: String,
    pub ast_description: AstDescription,
    pub language: String,
    pub tags: Vec<Tag>,
    #[serde(default)]
    pub robots: Vec<String>,
    pub copyright: Option<String>,
    pub id: String,
    pub title: String,
    pub provider: String,
    pub published: i64,
    pub source: String,
    pub urgency: i64,
    pub permission: Option<String>,
    pub story_path: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AstDescription {
    #[serde(rename = "type")]
    pub type_field: String,
    pub children: Vec<Children>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Children {
    #[serde(rename = "type")]
    pub type_field: String,
    pub children: Vec<Value>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Tag {
    pub title: String,
    pub args: Vec<Arg>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Arg {
    pub id: String,
    pub value: String,
}
