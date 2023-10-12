use crate::{
    Result,
    Error,
    chart::{
        SymbolInfo,
        SeriesCompletedMessage,
        session::{ ChartCallbackFn, WebSocket },
        ChartOptions,
        ChartSeries,
    },
    models::Interval,
    socket::DataServer,
};

use tracing::info;
use iso_currency::Currency;

#[derive(Default)]
pub struct Collector {
    ticker: String,
    interval: Interval,
    websocket: Option<WebSocket>,
    currency: Option<Currency>,
    from_time: i64,
}

impl Collector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_ticker(&mut self, symbol: &str, exchange: &str, interval: Interval) -> &mut Self {
        self.ticker = format!("{}:{}", exchange, symbol);
        self.interval = interval;
        self
    }

    pub async fn connect(&mut self, auth_token: &str, server: &DataServer) -> Result<&mut Self> {
        let handlers = ChartCallbackFn {
            on_chart_data: Box::new(|data| { Box::pin(Self::on_chart_data(data)) }),
            on_symbol_resolved: Box::new(|data| Box::pin(Self::on_symbol_resolved(data))),
            on_series_completed: Box::new(|data| Box::pin(Self::on_series_completed(data))),
        };
        let socket = WebSocket::build()
            .server(server.clone())
            .auth_token(auth_token.to_string())
            .connect(handlers).await?;
        self.websocket = Some(socket);
        Ok(self)
    }

    pub async fn collect(&mut self, time_step: i64) -> Result<()> {
        if self.websocket.is_none() {
            return Err(Error::Generic("websocket is not initialized".to_string()));
        }
        let socket = self.websocket.as_mut().unwrap();

        socket.set_market(&self.ticker, ChartOptions {
            resolution: self.interval.clone(),
            bar_count: 50_000,
            // replay_mode: Some(true),
            // replay_from: Some(self.from_time), //1689552000 // 1688342400 // 1687132800
            currency: self.currency.clone(),
            ..Default::default()
        }).await?;

        socket.subscribe().await;

        Ok(())
    }

    async fn on_chart_data(data: ChartSeries) {
        let end = data.data.first().unwrap().timestamp;
        info!("on_chart_data: {:?} - {:?}", data.data.len(), end);
        // info!("on_chart_data: {:?} - {:?}", data.data.len(), end);
    }

    async fn on_symbol_resolved(data: SymbolInfo) {
        info!("on_symbol_resolved: {:?}", data);
    }

    async fn on_series_completed(data: SeriesCompletedMessage) {
        info!("on_series_completed: {:?}", data);
    }
}
