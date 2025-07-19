// use dashmap::DashMap;
// use std::{
//     collections::VecDeque,
//     sync::{Arc, atomic::AtomicUsize},
// };
// use tokio::sync::mpsc::UnboundedSender;
// use ustr::Ustr;

// use crate::{
//     live::handler::{EventHandler, command::Command},
//     utils::gen_id,
//     websocket::WebSocketClient,
// };

// pub struct HistoricalDataClient {
//     series_count: Arc<AtomicUsize>,
//     study_count: Arc<AtomicUsize>,
//     session_count: Arc<AtomicUsize>,

//     ws: Option<WebSocketClient>,

//     study_map: Arc<DashMap<Ustr, Ustr>>,
//     series_map: Arc<DashMap<Ustr, Ustr>>,
//     event_handlers: Arc<EventHandler>,

//     command_tx: Option<UnboundedSender<Command>>,

//     symbol_deque: Arc<VecDeque<Ustr>>,
// }

// impl HistoricalDataClient {
//     pub fn new(auth_token: Option<&str>) -> Arc<Self> {
//         let series_count = Arc::new(AtomicUsize::new(0));
//         let study_count = Arc::new(AtomicUsize::new(0));
//         let session_count = Arc::new(AtomicUsize::new(0));

//         let ws = WebSocketClient::builder().maybe_auth_token(auth_token);
//         // let study_map = Arc::new(DashMap::new());
//         // let series_map = Arc::new(DashMap::new());

//         todo!()
//     }

//     pub async fn set_instrument(&self) {}

//     pub async fn set_replay(&self) {
//         let replay_series_id = gen_id();
//     }
//     pub async fn set_study() {}
// }

// // #[tracing::instrument(skip(self), level = "debug")]
// // #[builder]
// // pub async fn set_replay(
// //     &self,
// //     symbol: &str,
// //     options: ChartOptions,
// //     chart_session: &str,
// //     symbol_series_id: &str,
// // ) -> Result<()> {
// //     let replay_series_id = gen_id();
// //     let replay_session = gen_session_id("rs");

// //     self.create_replay_session(&replay_session).await?;
// //     self.add_replay_series()
// //         .chart_session(&replay_session)
// //         .series_id(&replay_series_id)
// //         .instrument(symbol)
// //         .interval(options.interval)
// //         .maybe_adjustment(options.adjustment)
// //         .maybe_currency(options.currency)
// //         .maybe_session_type(options.session_type)
// //         .call()
// //         .await?;

// //     self.replay_reset(&replay_session, &replay_series_id, options.replay_from)
// //         .await?;

// //     self.resolve_symbol()
// //         .symbol(options.symbol.as_str())
// //         .session(chart_session)
// //         .symbol_series_id(symbol_series_id)
// //         .maybe_adjustment(options.adjustment)
// //         .maybe_currency(options.currency)
// //         .replay_session(&replay_session)
// //         .call()
// //         .await?;

// //     Ok(())
// // }

// // pub async fn set_study(
// //     &self,
// //     study: StudyOptions,
// //     chart_session: &str,
// //     series_id: &str,
// // ) -> Result<()> {
// //     let study_count = self.studies_count.fetch_add(1, Ordering::SeqCst) + 1;

// //     let study_id = Ustr::from(&format!("st{study_count}"));

// //     let indicator = PineIndicator::build()
// //         .fetch(&study.script_id, &study.script_version, study.script_type)
// //         .await?;

// //     self.data_handler
// //         .metadata
// //         .studies
// //         .insert(indicator.metadata.data.id, study_id);

// //     // self.create_study(chart_session, &study_id, series_id, indicator)
// //     //     .await?;
// //     Ok(())
// // }

// // pub async fn set_market(&self, options: ChartOptions) -> Result<()> {
// //     let series_count = self.series_count.fetch_add(1, Ordering::SeqCst) + 1;
// //     let symbol_series_id = format!("sds_sym_{series_count}");
// //     let series_identifier = Ustr::from(&format!("sds_{series_count}"));
// //     let series_id = format!("s{series_count}");
// //     let chart_session = Ustr::from(&gen_session_id("cs"));
// //     let symbol = format!("{}:{}", options.exchange, options.symbol);
// //     self.create_chart_session(&chart_session).await?;

// //     if options.replay_mode {
// //         self.set_replay()
// //             .symbol(&symbol)
// //             .options(options)
// //             .chart_session(&chart_session)
// //             .symbol_series_id(&symbol_series_id)
// //             .call()
// //             .await?;
// //     } else {
// //         self.resolve_symbol()
// //             .session(&chart_session)
// //             .symbol_series_id(&symbol_series_id)
// //             .symbol(&symbol)
// //             .maybe_adjustment(options.adjustment)
// //             .maybe_currency(options.currency)
// //             .maybe_session_type(options.session_type)
// //             .call()
// //             .await?;
// //     }

// //     self.create_series()
// //         .chart_session(&chart_session)
// //         .series_identifier(&series_identifier)
// //         .series_id(&series_id)
// //         .symbol_series_id(&symbol_series_id)
// //         .interval(options.interval)
// //         .bar_count(options.bar_count)
// //         .maybe_range(options.range)
// //         .call()
// //         .await?;

// //     if let Some(study) = options.study_config {
// //         self.set_study(study, &chart_session, &series_identifier)
// //             .await?;
// //     }

// //     let series_info = SeriesInfo {
// //         chart_session,
// //         options,
// //     };

// //     self.data_handler
// //         .metadata
// //         .series
// //         .insert(series_identifier, series_info);

// //     Ok(())
// // }
