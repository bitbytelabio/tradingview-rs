use serde_json::Value;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::{
    Error,
    live::{handler::command::Command, models::TradingViewDataEvent},
};

pub type CommandTx = UnboundedSender<Command>;
pub type CommandRx = UnboundedReceiver<Command>;

pub trait Handler: Clone + Send + Sync + 'static {
    fn new(command_tx: CommandTx) -> Self;
    fn handle_events(&self, event: TradingViewDataEvent, message: &[Value]);
    fn handle_quote_data(&self, message: &[Value]);
    fn handle_series_data(&self, event: TradingViewDataEvent, messages: &[Value]);
    fn notify_error(&self, error: Error, message: &[Value]);
}
