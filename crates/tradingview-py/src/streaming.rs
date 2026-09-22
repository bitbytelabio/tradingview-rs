use serde::Deserialize;
use serde_json::Value;
use tokio::sync::mpsc::Sender;
use tradingview::Error;
use tradingview::chart::{DataPoint, OHLCV};
use tradingview::live::{handler::Handler, models::TradingViewDataEvent};

use crate::callbacks::CallbackDispatcher;
use crate::models::candle::CandleUpdate;
use crate::models::enums::Interval;
use crate::models::quote::QuoteTick;

pub struct QuoteStreamHandler {
    pub(crate) tx: Sender<QuoteTick>,
    pub(crate) dispatcher: CallbackDispatcher,
}

impl Handler for QuoteStreamHandler {
    fn handle_events(&self, event: TradingViewDataEvent, message: &[Value]) {
        if event == TradingViewDataEvent::OnQuoteData
            && message.len() >= 2
            && let Some(obj) = message[1].as_object()
        {
            let symbol = obj
                .get("n")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();
            if let Some(v) = obj.get("v").and_then(|v| v.as_object()) {
                let price = v
                    .get("lp")
                    .and_then(|v| v.as_f64())
                    .or_else(|| v.get("bid").and_then(|v| v.as_f64()))
                    .or_else(|| v.get("ask").and_then(|v| v.as_f64()))
                    .unwrap_or(0.0);
                let volume = v.get("volume").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let bid = v.get("bid").and_then(|v| v.as_f64());
                let ask = v.get("ask").and_then(|v| v.as_f64());
                let change = v.get("ch").and_then(|v| v.as_f64());
                let change_percent = v.get("chp").and_then(|v| v.as_f64());
                let timestamp = v
                    .get("lp_time")
                    .and_then(|v| v.as_f64())
                    .map(|t| t as i64)
                    .unwrap_or_else(|| chrono::Utc::now().timestamp());

                let tick = QuoteTick {
                    symbol,
                    timestamp,
                    price,
                    volume,
                    bid,
                    ask,
                    change,
                    change_percent,
                };

                self.dispatcher.dispatch(tick.clone());
                // Lossy drop-oldest queue policy (try_send drops item if buffer capacity 1024 is full)
                let _ = self.tx.try_send(tick);
            }
        }
    }

    fn handle_quote_data(&self, message: &[Value]) {
        self.handle_events(TradingViewDataEvent::OnQuoteData, message);
    }

    fn handle_series_data(&self, _event: TradingViewDataEvent, _messages: &[Value]) {}

    fn notify_error(&self, _error: Error, _message: &[Value]) {}
}

pub struct CandleStreamHandler {
    pub(crate) tx: Sender<CandleUpdate>,
    pub(crate) dispatcher: CallbackDispatcher,
    pub(crate) interval: Interval,
    pub(crate) default_symbol: String,
}

impl Handler for CandleStreamHandler {
    fn handle_events(&self, event: TradingViewDataEvent, message: &[Value]) {
        if (event == TradingViewDataEvent::OnChartData
            || event == TradingViewDataEvent::OnChartDataUpdate)
            && message.len() >= 2
            && let Some(obj) = message[1].as_object()
        {
            for (_key, series_val) in obj {
                if let Some(s_arr) = series_val.get("s").and_then(|v| v.as_array()) {
                    for item in s_arr {
                        if let Ok(dp) = DataPoint::deserialize(item) {
                            let candle = CandleUpdate {
                                symbol: self.default_symbol.clone(),
                                interval: self.interval,
                                timestamp: dp.timestamp(),
                                open: dp.open(),
                                high: dp.high(),
                                low: dp.low(),
                                close: dp.close(),
                                volume: dp.volume(),
                            };

                            self.dispatcher.dispatch(candle.clone());
                            let tx = self.tx.clone();
                            // Lossless backpressured queue policy: await capacity
                            tokio::spawn(async move {
                                let _ = tx.send(candle).await;
                            });
                        }
                    }
                }
            }
        }
    }

    fn handle_quote_data(&self, _message: &[Value]) {}
    fn handle_series_data(&self, _event: TradingViewDataEvent, _messages: &[Value]) {}
    fn notify_error(&self, _error: Error, _message: &[Value]) {}
}
