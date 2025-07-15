use bon::Builder;
use iso_currency::Currency;
use serde::{Deserialize, Serialize};
use ustr::Ustr;

use crate::{
    Interval, MarketAdjustment, SessionType, Timezone, options::Range, study::StudyConfiguration,
};

#[derive(Debug, Clone, Serialize, Deserialize, Copy, Builder)]
#[builder(on(Ustr, into))]
pub struct CommandMsg {
    pub inner: Ustr,
}

#[derive(Debug, Clone, Serialize, Deserialize, Builder)]
#[builder(on(Ustr, into))]
pub struct QuoteCommandMsg {
    pub quote_session: Ustr,
    pub symbols: Vec<Ustr>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Copy, Builder)]
#[builder(on(Ustr, into))]
pub struct AddReplaySeriesCommandMsg {
    chart_session: Ustr,
    series_id: Ustr,
    instrument: Ustr, // e.g., "HOSE:FPT"
    adjustment: Option<MarketAdjustment>,
    session_type: Option<SessionType>,
    currency: Option<Currency>,
    interval: Interval,
}

#[derive(Debug, Clone, Serialize, Deserialize, Copy, Builder)]
#[builder(on(Ustr, into))]
pub struct ReplayStepCommandMsg {
    pub chart_session: Ustr,
    pub series_id: Ustr,
    pub step: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Copy, Builder)]
#[builder(on(Ustr, into))]
pub struct ReplayStartCommandMsg {
    pub chart_session: Ustr,
    pub series_id: Ustr,
    pub interval: Interval,
}

#[derive(Debug, Clone, Serialize, Deserialize, Copy, Builder)]
#[builder(on(Ustr, into))]
pub struct SessionTerminationCommandMsg {
    pub chart_session: Ustr,
    pub id: Ustr,
}

#[derive(Debug, Clone, Serialize, Deserialize, Copy, Builder)]
#[builder(on(Ustr, into))]
pub struct ReplayResetCommandMsg {
    pub chart_session: Ustr,
    pub series_id: Ustr,
    pub timestamp: i64, // Reset to this timestamp in seconds
}

#[derive(Debug, Clone, Serialize, Deserialize, Builder)]
#[builder(on(Ustr, into))]
pub struct StudyCommandMsg {
    pub chart_session: Ustr,
    pub study_ids: [Ustr; 2],
    pub chart_series_id: Ustr,
    pub study: StudyConfiguration,
}

#[derive(Debug, Clone, Serialize, Deserialize, Copy, Builder)]
#[builder(on(Ustr, into))]
pub struct ChartSeriesCommandMsg {
    pub chart_session: Ustr,
    pub series_identifier: Ustr, // (sds_2)
    pub series_id: Ustr,         // (s1)
    pub symbol_series_id: Ustr,  // (sds_sym_2)
    pub interval: Interval,
    pub bar_count: u64,
    pub range: Option<Range>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Copy, Builder)]
#[builder(on(Ustr, into))]
pub struct ResolveSymbolCommandMsg {
    pub session: Ustr,
    pub symbol_series_id: Ustr,
    pub instrument: Ustr, // e.g., "HOSE:FPT"
    pub adjustment: Option<MarketAdjustment>,
    pub currency: Option<Currency>,
    pub session_type: Option<SessionType>,
    pub replay_session: Option<Ustr>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Copy, Builder)]
#[builder(on(Ustr, into))]
pub struct ChartDataRequestMsg {
    pub chart_session: Ustr,
    pub series_id: Ustr,
    pub num: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Copy, Builder)]
#[builder(on(Ustr, into))]
pub struct SetTimeZoneCommandMsg {
    pub chart_session: Ustr,
    pub timezone: Timezone, // e.g., "America/New_York"
}

#[derive(Debug, Clone, Serialize, Deserialize, Copy, Builder)]
#[builder(on(Ustr, into))]
pub struct SetLocaleCommandMsg {
    pub language: Ustr, // e.g., "en"
    pub country: Ustr,  // e.g., "US"
}
