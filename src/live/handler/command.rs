use crate::{Error, Result, live::handler::message::Command};
use backoff::{ExponentialBackoff, backoff::Backoff};
use std::sync::Arc;
use tokio::{
    select,
    time::{Duration, interval, sleep},
};
use tokio_util::sync::CancellationToken;

use crate::{live::handler::types::CommandRx, websocket::WebSocketClient};

pub struct CommandRunner {
    rx: CommandRx,
    ws: Arc<WebSocketClient>,
    shutdown: CancellationToken,
}

impl CommandRunner {
    pub fn new(rx: CommandRx, ws: Arc<WebSocketClient>) -> Self {
        Self {
            rx,
            ws,
            shutdown: CancellationToken::new(),
        }
    }

    pub async fn run(mut self) -> Result<()> {
        let mut hb = interval(Duration::from_secs(10));
        let mut backoff = ExponentialBackoff {
            max_elapsed_time: Some(Duration::from_secs(60)),
            ..Default::default()
        };

        loop {
            select! {
                // ❶ Application commands
                cmd = self.rx.recv() => match cmd {
                    Some(cmd) => self.process(cmd).await?,
                    None => break, // all senders dropped → shut down
                },

                // ❷ Detect a low-level close / socket error
                _ = self.ws.closed_notifier() => {
                    self.reconnect_with_backoff(&mut backoff).await?;
                },

                // ❸ Heartbeat (ping every N seconds)
                _ = hb.tick() => self.ws.try_ping().await?,

                // ❹ External shutdown signal
                _ = self.shutdown.cancelled() => break,
            }
        }
        Ok(())
    }

    async fn process(&self, cmd: Command) -> Result<()> {
        use Command::*;
        match cmd {
            Delete => self.ws.delete().await,
            SetAuthToken { auth_token } => self.ws.set_auth_token(&auth_token).await,
            /* … unchanged match arms … */
            _ => Ok(()), // default no-op
        }
    }

    async fn reconnect_with_backoff(&self, backoff: &mut impl Backoff) -> Result<()> {
        while let Some(delay) = backoff.next_backoff() {
            if self.ws.reconnect().await.is_ok() {
                tracing::info!("socket re-connected");
                return Ok(());
            }
            tracing::warn!("reconnect failed – retrying in {delay:?}");
            sleep(delay).await;
        }
        Err(Error::Internal("reconnect back-off exhausted".into()))
    }
}
