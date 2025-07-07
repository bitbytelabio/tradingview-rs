use crate::handler::message::TradingViewCommand;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

pub type CommandTx = UnboundedSender<TradingViewCommand>;
pub type CommandRx = UnboundedReceiver<TradingViewCommand>;

#[derive(Clone)]
pub struct TradingViewCommandHandler {
    pub command_tx: CommandTx,
}
