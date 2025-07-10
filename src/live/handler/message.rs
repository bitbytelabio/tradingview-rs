use crate::Timezone;
use serde::{Deserialize, Serialize};
use ustr::Ustr;

#[derive(Debug, Clone, Serialize, Deserialize, Copy)]
pub enum Command {
    Delete,
    Ping,
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
    CreateQuoteSession {
        quote_session: Ustr,
    },
}
