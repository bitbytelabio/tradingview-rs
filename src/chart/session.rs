use crate::{
    chart::{ChartOptions, StudyOptions},
    data_loader::DataLoader,
    models::{pine_indicator::PineIndicator, Interval, Timezone},
    payload,
    socket::{Socket, SocketMessageDe, SocketSession, TradingViewDataEvent},
    utils::{gen_id, gen_session_id, symbol_init},
    Result,
};
use async_trait::async_trait;
use serde_json::Value;
use std::fmt::Debug;

#[derive(Clone)]
pub struct WebSocket<'a> {
    data_loader: DataLoader<'a>,
    socket: SocketSession,
}

#[derive(Debug, Clone, Default)]
pub struct SeriesInfo {
    pub chart_session: String,
    pub options: ChartOptions,
}

impl<'a> WebSocket<'a> {
    pub fn new(data_loader: DataLoader<'a>, socket: SocketSession) -> Self {
        Self {
            data_loader,
            socket,
        }
    }

    // Begin TradingView WebSocket methods

    pub async fn set_locale(&mut self) -> Result<&mut Self> {
        self.socket
            .send("set_locale", &payload!("en", "US"))
            .await?;
        Ok(self)
    }

    pub async fn set_data_quality(&mut self, data_quality: &str) -> Result<&mut Self> {
        self.socket
            .send("set_data_quality", &payload!(data_quality))
            .await?;

        Ok(self)
    }

    pub async fn set_timezone(&mut self, session: &str, timezone: Timezone) -> Result<&mut Self> {
        self.socket
            .send("switch_timezone", &payload!(session, timezone.to_string()))
            .await?;

        Ok(self)
    }

    pub async fn update_auth_token(&mut self, auth_token: &str) -> Result<&mut Self> {
        self.socket.update_auth_token(auth_token).await?;
        Ok(self)
    }

    pub async fn create_chart_session(&mut self, session: &str) -> Result<&mut Self> {
        self.socket
            .send("chart_create_session", &payload!(session))
            .await?;
        Ok(self)
    }

    pub async fn create_replay_session(&mut self, session: &str) -> Result<&mut Self> {
        self.socket
            .send("replay_create_session", &payload!(session))
            .await?;
        Ok(self)
    }

    pub async fn add_replay_series(
        &mut self,
        session: &str,
        series_id: &str,
        symbol: &str,
        config: &ChartOptions,
    ) -> Result<&mut Self> {
        self.socket
            .send(
                "replay_add_series",
                &payload!(
                    session,
                    series_id,
                    symbol_init(
                        symbol,
                        config.adjustment.clone(),
                        config.currency,
                        config.session_type.clone(),
                        None
                    )?,
                    config.interval.to_string()
                ),
            )
            .await?;
        Ok(self)
    }

    pub async fn delete_chart_session_id(&mut self, session: &str) -> Result<&mut Self> {
        self.socket
            .send("chart_delete_session", &payload!(session))
            .await?;
        Ok(self)
    }

    pub async fn delete_replay_session_id(&mut self, session: &str) -> Result<&mut Self> {
        self.socket
            .send("replay_delete_session", &payload!(session))
            .await?;
        Ok(self)
    }

    pub async fn replay_step(
        &mut self,
        session: &str,
        series_id: &str,
        step: u64,
    ) -> Result<&mut Self> {
        self.socket
            .send("replay_step", &payload!(session, series_id, step))
            .await?;
        Ok(self)
    }

    pub async fn replay_start(
        &mut self,
        session: &str,
        series_id: &str,
        interval: Interval,
    ) -> Result<&mut Self> {
        self.socket
            .send(
                "replay_start",
                &payload!(session, series_id, interval.to_string()),
            )
            .await?;
        Ok(self)
    }

    pub async fn replay_stop(&mut self, session: &str, series_id: &str) -> Result<&mut Self> {
        self.socket
            .send("replay_stop", &payload!(session, series_id))
            .await?;
        Ok(self)
    }

    pub async fn replay_reset(
        &mut self,
        session: &str,
        series_id: &str,
        timestamp: i64,
    ) -> Result<&mut Self> {
        self.socket
            .send("replay_reset", &payload!(session, series_id, timestamp))
            .await?;
        Ok(self)
    }

    pub async fn request_more_data(
        &mut self,
        session: &str,
        series_id: &str,
        num: u64,
    ) -> Result<&mut Self> {
        self.socket
            .send("request_more_data", &payload!(session, series_id, num))
            .await?;
        Ok(self)
    }

    pub async fn request_more_tickmarks(
        &mut self,
        session: &str,
        series_id: &str,
        num: u64,
    ) -> Result<&mut Self> {
        self.socket
            .send("request_more_tickmarks", &payload!(session, series_id, num))
            .await?;
        Ok(self)
    }

    pub async fn create_study(
        &mut self,
        session: &str,
        study_id: &str,
        series_id: &str,
        indicator: PineIndicator,
    ) -> Result<&mut Self> {
        let inputs = indicator.to_study_inputs()?;
        let payloads: Vec<Value> = vec![
            Value::from(session),
            Value::from(study_id),
            Value::from("st1"),
            Value::from(series_id),
            Value::from(indicator.script_type.to_string()),
            inputs,
        ];
        self.socket.send("create_study", &payloads).await?;
        Ok(self)
    }

    pub async fn modify_study(
        &mut self,
        session: &str,
        study_id: &str,
        series_id: &str,
        indicator: PineIndicator,
    ) -> Result<&mut Self> {
        let inputs = indicator.to_study_inputs()?;
        let payloads: Vec<Value> = vec![
            Value::from(session),
            Value::from(study_id),
            Value::from("st1"),
            Value::from(series_id),
            Value::from(indicator.script_type.to_string()),
            inputs,
        ];
        self.socket.send("modify_study", &payloads).await?;
        Ok(self)
    }

    pub async fn remove_study(&mut self, session: &str, study_id: &str) -> Result<&mut Self> {
        self.socket
            .send("remove_study", &payload!(session, study_id))
            .await?;
        Ok(self)
    }

    pub async fn create_series(
        &mut self,
        session: &str,
        series_id: &str,
        series_version: &str,
        series_symbol_id: &str,
        config: &ChartOptions,
    ) -> Result<&mut Self> {
        let range = match (&config.range, config.from, config.to) {
            (Some(range), _, _) => range.clone(),
            (None, Some(from), Some(to)) => format!("r,{}:{}", from, to),
            _ => String::default(),
        };
        self.socket
            .send(
                "create_series",
                &payload!(
                    session,
                    series_id,
                    series_version,
                    series_symbol_id,
                    config.interval.to_string(),
                    config.bar_count,
                    range // |r,1626220800:1628640000|1D|5d|1M|3M|6M|YTD|12M|60M|ALL|
                ),
            )
            .await?;
        Ok(self)
    }

    pub async fn modify_series(
        &mut self,
        session: &str,
        series_id: &str,
        series_version: &str,
        series_symbol_id: &str,
        config: &ChartOptions,
    ) -> Result<&mut Self> {
        let range = match (&config.range, config.from, config.to) {
            (Some(range), _, _) => range.clone(),
            (None, Some(from), Some(to)) => format!("r,{}:{}", from, to),
            _ => String::default(),
        };
        self.socket
            .send(
                "modify_series",
                &payload!(
                    session,
                    series_id,
                    series_version,
                    series_symbol_id,
                    config.interval.to_string(),
                    config.bar_count,
                    range // |r,1626220800:1628640000|1D|5d|1M|3M|6M|YTD|12M|60M|ALL|
                ),
            )
            .await?;
        Ok(self)
    }

    pub async fn remove_series(&mut self, session: &str, series_id: &str) -> Result<&mut Self> {
        self.socket
            .send("remove_series", &payload!(session, series_id))
            .await?;
        Ok(self)
    }

    pub async fn resolve_symbol(
        &mut self,
        session: &str,
        symbol_series_id: &str,
        symbol: &str,
        config: &ChartOptions,
        replay_session: Option<String>,
    ) -> Result<&mut Self> {
        self.socket
            .send(
                "resolve_symbol",
                &payload!(
                    session,
                    symbol_series_id,
                    symbol_init(
                        symbol,
                        config.adjustment.clone(),
                        config.currency,
                        config.session_type.clone(),
                        replay_session
                    )?
                ),
            )
            .await?;
        Ok(self)
    }

    pub async fn delete(&mut self) -> Result<&mut Self> {
        for (_, s) in self.data_loader.metadata.series.clone() {
            self.delete_chart_session_id(&s.chart_session).await?;
        }
        self.socket.close().await?;
        Ok(self)
    }

    // End TradingView WebSocket methods

    pub async fn set_replay(
        &mut self,
        symbol: &str,
        options: &ChartOptions,
        chart_session: &str,
        symbol_series_id: &str,
    ) -> Result<&mut Self> {
        let replay_series_id = gen_id();
        let replay_session = gen_session_id("rs");

        self.create_replay_session(&replay_session).await?;
        self.add_replay_series(&replay_session, &replay_series_id, symbol, options)
            .await?;
        self.replay_reset(&replay_session, &replay_series_id, options.replay_from)
            .await?;
        self.resolve_symbol(
            chart_session,
            symbol_series_id,
            &options.symbol,
            options,
            Some(replay_session.to_string()),
        )
        .await?;

        Ok(self)
    }

    pub async fn set_study(
        &mut self,
        study: &StudyOptions,
        chart_session: &str,
        series_id: &str,
    ) -> Result<&mut Self> {
        self.data_loader.metadata.studies_count += 1;
        let study_count = self.data_loader.metadata.studies_count;
        let study_id = format!("st{}", study_count);

        let indicator = PineIndicator::build()
            .fetch(
                &study.script_id,
                &study.script_version,
                study.script_type.clone(),
            )
            .await?;

        self.data_loader
            .metadata
            .studies
            .insert(indicator.metadata.data.id.clone(), study_id.clone());

        self.create_study(chart_session, &study_id, series_id, indicator)
            .await?;
        Ok(self)
    }

    pub async fn set_market(&mut self, options: ChartOptions) -> Result<&mut Self> {
        self.data_loader.metadata.series_count += 1;
        let series_count = self.data_loader.metadata.series_count;
        let symbol_series_id = format!("sds_sym_{}", series_count);
        let series_id = format!("sds_{}", series_count);
        let series_version = format!("s{}", series_count);
        let chart_session = gen_session_id("cs");

        self.create_chart_session(&chart_session).await?;

        if options.replay_mode {
            self.set_replay(&options.symbol, &options, &chart_session, &symbol_series_id)
                .await?;
        } else {
            self.resolve_symbol(
                &chart_session,
                &symbol_series_id,
                &options.symbol,
                &options,
                None,
            )
            .await?;
        }

        self.create_series(
            &chart_session,
            &series_id,
            &series_version,
            &symbol_series_id,
            &options,
        )
        .await?;

        if let Some(study) = &options.study_config {
            self.set_study(study, &chart_session, &series_id).await?;
        }

        let series_info = SeriesInfo {
            chart_session,
            options,
        };

        self.data_loader
            .metadata
            .series
            .insert(series_id, series_info);

        Ok(self)
    }

    pub async fn subscribe(&mut self) {
        self.event_loop(&mut self.socket.clone()).await;
    }
}

#[async_trait]
impl<'a> Socket for WebSocket<'a> {
    async fn handle_message_data(&mut self, message: SocketMessageDe) -> Result<()> {
        let event = TradingViewDataEvent::from(message.m.clone());
        self.data_loader.handle_events(event, &message.p).await;
        Ok(())
    }
}
