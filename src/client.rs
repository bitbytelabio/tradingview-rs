use crate::model::Indicator;
use crate::{errors, user::User};

#[derive(Debug)]
pub struct Client {
    user: User,
}

#[derive(Debug, PartialEq)]
pub enum Screener {
    America,
    Australia,
    Canada,
    Egypt,
    Germany,
    India,
    Israel,
    Italy,
    Luxembourg,
    Poland,
    Sweden,
    Turkey,
    UK,
    Vietnam,
    Other(String),
}

#[tracing::instrument]
fn get_screener(exchange: &str) -> Screener {
    let e = exchange.to_uppercase();
    match e.as_str() {
        "NASDAQ" | "NYSE" | "NYSE ARCA" | "OTC" => Screener::America,
        "ASX" => Screener::Australia,
        "TSX" | "TSXV" | "CSE" | "NEO" => Screener::Canada,
        "EGX" => Screener::Egypt,
        "FWB" | "SWB" | "XETR" => Screener::Germany,
        "BSE" | "NSE" => Screener::India,
        "TASE" => Screener::Israel,
        "MIL" | "MILSEDEX" => Screener::Italy,
        "LUXSE" => Screener::Luxembourg,
        "NEWCONNECT" => Screener::Poland,
        "NGM" => Screener::Sweden,
        "BIST" => Screener::Turkey,
        "LSE" | "LSIN" => Screener::UK,
        "HNX" | "HOSE" | "UPCOM" => Screener::Vietnam,
        _ => Screener::Other(exchange.to_lowercase()),
    }
}
impl Client {
    fn new(user: User) -> Self {
        Self { user }
    }

    async fn get(&self, url: &str) -> Result<reqwest::Response, Box<dyn std::error::Error>> {
        let cookie = format!(
            "sessionid={}; sessionid_sign={};",
            self.user.session, self.user.signature
        );
        let client: reqwest::Client = crate::utils::reqwest_build_client(Some(&cookie))?;
        let response = client.get(url).send().await?;
        // let data: serde_json::Value = response.json().await?;
        Ok(response)
    }

    async fn post<T: serde::Serialize>(
        &self,
        url: &str,
        body: T,
    ) -> Result<reqwest::Response, Box<dyn std::error::Error>> {
        let cookie = format!(
            "sessionid={}; sessionid_sign={};",
            self.user.session, self.user.signature
        );
        let client: reqwest::Client = crate::utils::reqwest_build_client(Some(&cookie))?;
        let response = client.post(url).json(&body).send().await?;
        Ok(response)
    }

    #[tracing::instrument(skip(self))]
    pub async fn get_chart_token(
        &self,
        layout_id: &str,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let data: serde_json::Value = self
            .get(&format!(
                "https://www.tradingview.com/chart-token/?image_url={}&user_id={}",
                layout_id, self.user.id
            ))
            .await?
            .json()
            .await?;

        match data.get("token") {
            Some(token) => {
                return Ok(match token.as_str() {
                    Some(token) => token.to_string(),
                    None => {
                        return Err(Box::new(errors::ClientError::NoTokenFound(
                            "Chart token not found".to_string(),
                        )))
                    }
                })
            }
            None => return Err("No token found").unwrap(),
        }
    }

    #[tracing::instrument(skip(self))]
    pub async fn get_drawing<T>(
        &self,
        layout_id: &str,
        symbol: &str,
        chart_id: T,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error>>
    where
        T: std::fmt::Display + std::fmt::Debug,
    {
        let token = self.get_chart_token(layout_id).await.unwrap();
        let url = format!(
            "https://charts-storage.tradingview.com/charts-storage/layout/{layout_id}/sources?chart_id={chart_id}&jwt={token}&symbol={symbol}",
            layout_id = layout_id,
            chart_id = chart_id,
            token = token,
            symbol = symbol
        );

        Ok(self.get(&url).await?.json().await?)
    }

    #[tracing::instrument(skip(self))]
    pub async fn get_private_indicators(
        &self,
    ) -> Result<Vec<Indicator>, Box<dyn std::error::Error>> {
        let indicators = self
            .get("https://pine-facade.tradingview.com/pine-facade/list?filter=saved")
            .await?
            .json::<Vec<Indicator>>()
            .await?;
        Ok(indicators)
    }

    #[tracing::instrument(skip(self))]
    pub async fn get_builtin_indicators(
        &self,
    ) -> Result<Vec<Indicator>, Box<dyn std::error::Error>> {
        let indicator_types = vec!["standard", "candlestick", "fundamental"];
        let mut indicators: Vec<Indicator> = vec![];
        for indicator_type in indicator_types {
            let url = format!(
                "https://pine-facade.tradingview.com/pine-facade/list/?filter={}",
                indicator_type
            );

            let mut data = self.get(&url).await?.json::<Vec<Indicator>>().await?;

            indicators.append(&mut data);
        }
        Ok(indicators)
    }

    #[tracing::instrument(skip(self))]
    pub async fn get_indicator_data(
        &self,
        indicator: &Indicator,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        let url = format!(
            "https://pine-facade.tradingview.com/pine-facade/translate/{}/{}",
            indicator.id, indicator.version
        );
        let data: serde_json::Value = self.get(&url).await?.json().await?;
        Ok(data)
    }

    #[tracing::instrument(skip(self))]
    pub async fn fetch_scan_data(
        &self,
        tickers: Vec<&str>,
        scan_type: &str,
        columns: Vec<&str>,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        let url = format!("https://scanner.tradingview.com/{}/scan", scan_type);
        let body = serde_json::json!({
            "symbols": {
                "tickers": tickers
            },
            "columns": columns
        });
        let data: serde_json::Value = self.post(&url, body).await?.json().await?;
        Ok(data)
    }
}
