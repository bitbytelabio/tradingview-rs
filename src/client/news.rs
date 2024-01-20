use crate::{
    utils::get, MarketType, News, NewsArea, NewsContent, NewsHeadlines, NewsSection, Result,
    UserCookies,
};

static BASE_NEWS_URL: &str = "https://news-headlines.tradingview.com/v2";

fn get_news_category<'a>(market_type: &'a MarketType) -> &'a str {
    match market_type {
        MarketType::All => "base",
        MarketType::Stocks(_) => "stock",
        MarketType::Funds(_) => "etf",
        MarketType::Futures => "futures",
        MarketType::Forex => "forex",
        MarketType::Crypto(_) => "crypto",
        MarketType::Indices => "index",
        MarketType::Bonds => "bond",
        MarketType::Economy => "economic",
    }
}

fn get_news_area<'a>(area: &'a NewsArea) -> &'a str {
    match area {
        NewsArea::World => "WLD",
        NewsArea::Americas => "AME",
        NewsArea::Europe => "EUR",
        NewsArea::Asia => "ASI",
        NewsArea::Oceania => "OCN",
        NewsArea::Africa => "AFR",
    }
}

fn get_news_section<'a>(section: &'a NewsSection) -> &'a str {
    match section {
        NewsSection::PressRelease => "press_release",
        NewsSection::FinancialStatement => "financial_statement",
        NewsSection::InsiderTrading => "insider_trading",
        NewsSection::ESG => "esg",
        NewsSection::CorpActivitiesAll => "corp_activity",
        NewsSection::AnalysisAll => "analysis",
        NewsSection::AnalysisRecommendations => "recommendation",
        NewsSection::EstimatesAndForecasts => "prediction",
        NewsSection::MarketToday => "markets_today",
        NewsSection::Surveys => "survey",
    }
}

pub async fn list_news(
    client: Option<&UserCookies>,
    category: &MarketType,
    area: Option<&NewsArea>,
    section: Option<&NewsSection>,
) -> Result<NewsHeadlines> {
    let category = get_news_category(category);
    let mut queries = vec![
        ("category", category),
        ("client", "web"),
        ("lang", "en"),
        ("streaming", "false"),
    ];
    if let Some(area) = area {
        queries.push(("area", get_news_area(area)));
    }
    if let Some(section) = section {
        queries.push(("section", get_news_section(section)));
    }
    let res = get(client, &format!("{BASE_NEWS_URL}/headlines"), &queries)
        .await?
        .json::<NewsHeadlines>()
        .await?;

    Ok(res)
}

async fn fetch_news(id: &str) -> Result<NewsContent> {
    // let id_url_encoded = urlencoding::encode(id);
    let res = get(
        None,
        &format!("{BASE_NEWS_URL}/story"),
        &[("id", id), ("lang", "en")],
    )
    .await?
    .json::<NewsContent>()
    .await?;

    Ok(res)
}

impl News {
    pub fn get_url(&self) -> String {
        format!("https://www.tradingview.com{}", self.story_path)
    }

    pub fn get_source_url(&self) -> String {
        if let Some(url) = &self.link {
            return url.to_string();
        }
        self.get_url()
    }

    pub fn get_related_symbols(&self) -> Vec<String> {
        self.related_symbols
            .iter()
            .map(|s| s.symbol.to_owned())
            .collect()
    }

    pub async fn get_content(&self) -> Result<NewsContent> {
        fetch_news(&self.id).await
    }
}

#[tokio::test]
async fn test_list_news() -> Result<()> {
    let res = list_news(
        None,
        &MarketType::All,
        None,
        Some(&NewsSection::AnalysisAll),
    )
    .await?;
    println!("{:#?}", res);
    Ok(())
}

#[tokio::test]
async fn test_fetch_news() -> Result<()> {
    let _ = fetch_news("tag:reuters.com,2024:newsml_L4N3E9476:0").await?;

    // let res = list_news(
    //     None,
    //     &MarketType::All,
    //     None,
    //     Some(&NewsSection::AnalysisAll),
    // )
    // .await?;

    // for item in res.items.iter() {
    //     let content = item.get_content().await.unwrap();
    //     println!("{:#?}", content);
    // }

    Ok(())
}