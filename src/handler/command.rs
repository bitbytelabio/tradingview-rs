use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::{Result, handler::message::TradingViewCommand};

pub type CommandTx = UnboundedSender<TradingViewCommand>;
pub type CommandRx = UnboundedReceiver<TradingViewCommand>;

use crate::client::websocket::WebSocketClient;

impl WebSocketClient {
    pub async fn handle_commands(&mut self) -> Result<()> {
        let mut command_rx = self.command_rx.lock().await;
        while let Some(command) = command_rx.recv().await {
            match command {
                TradingViewCommand::Cleanup => {
                    self.delete().await?;
                }
                TradingViewCommand::SetAuthToken { auth_token } => {
                    self.socket.set_auth_token(&auth_token).await?;
                }
                TradingViewCommand::SetLocals { locals } => {
                    self.set_locale((&locals.0, &locals.1)).await?;
                }
                TradingViewCommand::SetDataQuality { quality } => {
                    self.set_data_quality(&quality).await?;
                }
                TradingViewCommand::SetTimeZone { session, timezone } => {
                    self.set_timezone(&session, timezone).await?;
                }
                TradingViewCommand::CreateQuoteSession => {
                    self.create_quote_session().await?;
                }
                TradingViewCommand::DeleteQuoteSession => {
                    self.delete_quote_session().await?;
                }
                TradingViewCommand::SetQuoteFields => {
                    self.set_fields().await?;
                }
                TradingViewCommand::QuoteFastSymbols { symbols } => {
                    let symbols: Vec<_> = symbols.into_iter().map(|s| s.as_str()).collect();
                    self.fast_symbols(symbols).await?;
                }
                TradingViewCommand::QuoteRemoveSymbols { symbols } => {
                    let symbols: Vec<_> = symbols.into_iter().map(|s| s.as_str()).collect();
                    self.remove_symbols(symbols).await?;
                }
                TradingViewCommand::CreateChartSession { session } => {
                    self.create_chart_session(&session).await?;
                }
                TradingViewCommand::DeleteChartSession { session } => {
                    self.delete_chart_session(&session).await?;
                }
                TradingViewCommand::RequestMoreData {
                    session,
                    series_id,
                    bar_count,
                } => {
                    self.request_more_data(&session, &series_id, bar_count)
                        .await?;
                }
                TradingViewCommand::RequestMoreTickMarks {
                    session,
                    series_id,
                    bar_count,
                } => {
                    self.request_more_tickmarks(&session, &series_id, bar_count)
                        .await?;
                }
                TradingViewCommand::CreateStudy {
                    session,
                    study_id,
                    series_id,
                    indicator,
                } => {
                    self.create_study(&session, &study_id, &series_id, indicator)
                        .await?;
                }
                TradingViewCommand::ModifyStudy {
                    session,
                    study_id,
                    series_id,
                    indicator,
                } => {
                    self.modify_study(&session, &study_id, &series_id, indicator)
                        .await?;
                }
                TradingViewCommand::RemoveStudy {
                    session,
                    study_id,
                    series_id,
                } => {
                    let id = format!("{series_id}_{study_id}");
                    self.remove_study(&session, &id).await?;
                }
                TradingViewCommand::SetStudy {
                    study_options,
                    session,
                    series_id,
                } => {
                    self.set_study(study_options, &session, &series_id).await?;
                }
                TradingViewCommand::CreateSeries {
                    session,
                    series_id,
                    series_version,
                    series_symbol_id,
                    config,
                } => {
                    self.create_series(
                        &session,
                        &series_id,
                        &series_version,
                        &series_symbol_id,
                        config,
                    )
                    .await?;
                }
                TradingViewCommand::ModifySeries {
                    session,
                    series_id,
                    series_version,
                    series_symbol_id,
                    config,
                } => {
                    self.modify_series(
                        &session,
                        &series_id,
                        &series_version,
                        &series_symbol_id,
                        config,
                    )
                    .await?;
                }
                TradingViewCommand::RemoveSeries { session, series_id } => {
                    self.remove_series(&session, &series_id).await?;
                }
                TradingViewCommand::CreateReplaySession { session } => {
                    self.create_replay_session(&session).await?;
                }
                TradingViewCommand::DeleteReplaySession { session } => {
                    self.delete_replay_session(&session).await?;
                }
                TradingViewCommand::ResolveSymbol {
                    session,
                    symbol,
                    exchange,
                    opts,
                    replay_session,
                } => {
                    self.resolve_symbol(
                        &session,
                        &symbol,
                        &exchange,
                        opts,
                        replay_session.as_deref(),
                    )
                    .await?;
                }
                TradingViewCommand::SetReplayStep {
                    session,
                    series_id,
                    step,
                } => {
                    self.replay_step(&session, &series_id, step).await?;
                }
                TradingViewCommand::StartReplay {
                    session,
                    series_id,
                    interval,
                } => {
                    self.replay_start(&session, &series_id, interval).await?;
                }
                TradingViewCommand::StopReplay { session, series_id } => {
                    self.replay_stop(&session, &series_id).await?;
                }
                TradingViewCommand::ResetReplay {
                    session,
                    series_id,
                    timestamp,
                } => {
                    self.replay_reset(&session, &series_id, timestamp).await?;
                }
                TradingViewCommand::SetReplay {
                    symbol,
                    options,
                    chart_session,
                    symbol_series_id,
                } => {
                    self.set_replay(&symbol, options, &chart_session, &symbol_series_id)
                        .await?;
                }
                TradingViewCommand::SetMarket { options } => {
                    self.set_market(options).await?;
                }
            }
        }
        Ok(())
    }
}
