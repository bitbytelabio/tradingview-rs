// Macro for creating simple command messages
#[macro_export]
macro_rules! cmd_msg {
    ($value:expr) => {
        CommandMsg {
            inner: ustr::ustr($value),
        }
    };
}

// Macro for creating quote command messages
#[macro_export]
macro_rules! quote_cmd {
    ($session:expr, $($symbol:expr),+ $(,)?) => {
        QuoteCommandMsg {
            quote_session: ustr::ustr($session),
            symbols: vec![$(ustr::ustr($symbol)),+],
        }
    };
}

// Macro for creating chart series command messages
#[macro_export]
macro_rules! chart_series_cmd {
    (
        session: $session:expr,
        series_id: $series_id:expr,
        symbol_series_id: $symbol_series_id:expr,
        series_identifier: $series_identifier:expr,
        interval: $interval:expr,
        bar_count: $bar_count:expr
        $(, range: $range:expr)?
    ) => {
        ChartSeriesCommandMsg {
            chart_session: ustr::ustr($session),
            series_identifier: ustr::ustr($series_identifier),
            series_id: ustr::ustr($series_id),
            symbol_series_id: ustr::ustr($symbol_series_id),
            interval: $interval,
            bar_count: $bar_count,
            range: None $(.or(Some($range)))?,
        }
    };
}

// Macro for creating resolve symbol command messages
#[macro_export]
macro_rules! resolve_symbol_cmd {
    (
        session: $session:expr,
        symbol_series_id: $symbol_series_id:expr,
        instrument: $instrument:expr
        $(, adjustment: $adjustment:expr)?
        $(, currency: $currency:expr)?
        $(, session_type: $session_type:expr)?
        $(, replay_session: $replay_session:expr)?
    ) => {
        ResolveSymbolCommandMsg {
            session: ustr::ustr($session),
            symbol_series_id: ustr::ustr($symbol_series_id),
            instrument: ustr::ustr($instrument),
            adjustment: None $(.or(Some($adjustment)))?,
            currency: None $(.or(Some($currency)))?,
            session_type: None $(.or(Some($session_type)))?,
            replay_session: None $(.or(Some(ustr::ustr($replay_session))))?,
        }
    };
}

// Macro for creating study command messages
#[macro_export]
macro_rules! study_cmd {
    (
        session: $session:expr,
        study_ids: [$id1:expr, $id2:expr],
        chart_series_id: $chart_series_id:expr,
        study: $study:expr
    ) => {
        StudyCommandMsg {
            chart_session: ustr::ustr($session),
            study_ids: [ustr::ustr($id1), ustr::ustr($id2)],
            chart_series_id: ustr::ustr($chart_series_id),
            study: $study,
        }
    };
}

// Macro for creating replay series command messages
#[macro_export]
macro_rules! replay_series_cmd {
    (
        replay_session: $replay_session:expr,
        request_id: $request_id:expr,
        instrument: $instrument:expr,
        interval: $interval:expr
        $(, adjustment: $adjustment:expr)?
        $(, currency: $currency:expr)?
        $(, session_type: $session_type:expr)?
    ) => {
        AddReplaySeriesCommandMsg {
            replay_session: ustr::ustr($replay_session),
            request_id: ustr::ustr($request_id),
            instrument: ustr::ustr($instrument),
            interval: $interval,
            adjustment: None $(.or(Some($adjustment)))?,
            currency: None $(.or(Some($currency)))?,
            session_type: None $(.or(Some($session_type)))?,
        }
    };
}

// Macro for creating session termination messages
#[macro_export]
macro_rules! session_term_cmd {
    ($session:expr, $id:expr) => {
        SessionTerminationCommandMsg {
            chart_session: ustr::ustr($session),
            id: ustr::ustr($id),
        }
    };
}

// Macro for creating locale command messages
#[macro_export]
macro_rules! locale_cmd {
    ($lang:expr, $country:expr) => {
        SetLocaleCommandMsg {
            language: ustr::ustr($lang),
            country: ustr::ustr($country),
        }
    };
}

// Macro for creating timezone command messages
#[macro_export]
macro_rules! timezone_cmd {
    ($session:expr, $timezone:expr) => {
        SetTimeZoneCommandMsg {
            chart_session: ustr::ustr($session),
            timezone: $timezone,
        }
    };
}

#[cfg(test)]
mod tests {
    use crate::live::handler::message::AddReplaySeriesCommandMsg;
    use crate::models::{Interval, MarketAdjustment, SessionType};
    use iso_currency::Currency;
    use ustr::ustr;

    #[test]
    fn test_replay_series_cmd_expansion() {
        let cmd = replay_series_cmd!(
            replay_session: "rep_123",
            request_id: "req_456",
            instrument: "BINANCE:BTCUSDT",
            interval: Interval::OneMinute,
            adjustment: MarketAdjustment::Splits,
            currency: Currency::USD,
            session_type: SessionType::Regular
        );

        assert_eq!(cmd.replay_session, ustr("rep_123"));
        assert_eq!(cmd.request_id, ustr("req_456"));
        assert_eq!(cmd.instrument, ustr("BINANCE:BTCUSDT"));
        assert_eq!(cmd.interval, Interval::OneMinute);
        assert!(matches!(cmd.adjustment, Some(MarketAdjustment::Splits)));
        assert_eq!(cmd.currency, Some(Currency::USD));
        assert_eq!(cmd.session_type, Some(SessionType::Regular));

        let cmd_minimal = replay_series_cmd!(
            replay_session: "rep_789",
            request_id: "req_012",
            instrument: "HOSE:FPT",
            interval: Interval::OneDay
        );

        assert_eq!(cmd_minimal.replay_session, ustr("rep_789"));
        assert_eq!(cmd_minimal.request_id, ustr("req_012"));
        assert_eq!(cmd_minimal.instrument, ustr("HOSE:FPT"));
        assert_eq!(cmd_minimal.interval, Interval::OneDay);
        assert!(cmd_minimal.adjustment.is_none());
        assert_eq!(cmd_minimal.currency, None);
        assert_eq!(cmd_minimal.session_type, None);
    }
}
