use crate::{utils::get, NewsHeadlines, Result, UserCookies};

static BASE_NEWS_URL: &str = "https://news-headlines.tradingview.com/v2";

pub async fn list_news(client: Option<&UserCookies>) -> Result<NewsHeadlines> {
    let queries = vec![
        ("category", ""),
        ("client", "web"),
        ("lang", "en"),
        ("streaming", "true"),
    ];

    let res = get(
        client,
        &format!("{BASE_NEWS_URL}/headlines"),
        Some(&queries),
    )
    .await?
    .json::<NewsHeadlines>()
    .await?;

    Ok(res)
}

#[tokio::test]
async fn test_list_news() -> Result<()> {
    let res = list_news(None).await?;
    println!("{:#?}", res);
    Ok(())
}
