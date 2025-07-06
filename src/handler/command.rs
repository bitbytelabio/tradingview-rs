use crate::ChartOptions;
use serde::{Deserialize, Serialize};
use ustr::Ustr;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TradingViewCommand {
    CloseSocket,
    CreateQuoteSession,
    DeleteQuoteSession,
    SetQuoteFields,
    SetAuthToken {
        auth_token: Ustr,
    },
    QuoteFastSymbols {
        symbols: Vec<Ustr>,
    },
    CreateChartSession {
        chart_session: Ustr,
        options: ChartOptions,
    },
}
