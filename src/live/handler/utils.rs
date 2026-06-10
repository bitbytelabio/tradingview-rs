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
        session: $session:expr,
        series_id: $series_id:expr,
        instrument: $instrument:expr,
        interval: $interval:expr
        $(, adjustment: $adjustment:expr)?
        $(, currency: $currency:expr)?
        $(, session_type: $session_type:expr)?
    ) => {
        AddReplaySeriesCommandMsg {
            chart_session: ustr::ustr($session),
            series_id: ustr::ustr($series_id),
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
