use crate::Timezone;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use ustr::Ustr;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Command {
    Close,
    Ping,
    Send {
        message: Ustr,
        payload: Vec<Value>,
    },
    SendRawMessage {
        message: Ustr,
    },
    SetAuthToken {
        auth_token: Ustr,
    },
    SetLocale {
        language: Ustr,
        country: Ustr,
    },
    SetDataQuality {
        quality: Ustr,
    },
    SetTimeZone {
        chart_session: Ustr,
        timezone: Timezone,
    },

    // Quote Session Commands
    CreateQuoteSession {
        quote_session: Ustr,
    },
    DeleteQuoteSession {
        quote_session: Ustr,
    },
    FastSymbols {
        symbols: Vec<Ustr>,
    },
    SetQuoteFields {
        quote_session: Ustr,
    },
    AddQuoteSymbols {
        quote_session: Ustr,
        symbols: Vec<Ustr>,
    },
    RemoveQuoteSymbols {
        quote_session: Ustr,
        symbols: Vec<Ustr>,
    },

    /// Chart Session Commands
    CreateChartSession {
        chart_session: Ustr,
    },
    DeleteChartSession {
        chart_session: Ustr,
    },
}
