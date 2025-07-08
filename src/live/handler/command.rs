use crate::{Error, Result, live::handler::message::Command};
use std::sync::Arc;
use tokio::{
    select,
    time::{Duration, interval, sleep},
};
use tokio_util::sync::CancellationToken;

use crate::{live::handler::types::CommandRx, websocket::WebSocketClient};

/// Exponential backoff strategy for reconnection attempts
struct ExponentialBackoff {
    current_delay: Duration,
    max_delay: Duration,
    max_attempts: usize,
    attempts: usize,
    multiplier: f64,
}

impl ExponentialBackoff {
    fn new() -> Self {
        Self {
            current_delay: Duration::from_millis(1000), // Start with 1 second
            max_delay: Duration::from_secs(60),         // Cap at 60 seconds
            max_attempts: 10,                           // Max 10 attempts
            attempts: 0,
            multiplier: 2.0, // Double each time
        }
    }

    fn next_backoff(&mut self) -> Option<Duration> {
        if self.attempts >= self.max_attempts {
            return None;
        }

        let delay = self.current_delay;
        self.attempts += 1;

        // Exponential backoff with jitter
        self.current_delay = std::cmp::min(
            Duration::from_millis((self.current_delay.as_millis() as f64 * self.multiplier) as u64),
            self.max_delay,
        );

        Some(delay)
    }

    fn reset(&mut self) {
        self.current_delay = Duration::from_millis(1000);
        self.attempts = 0;
    }
}

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
        let mut backoff = ExponentialBackoff::new();

        loop {
            select! {
                // ❶ Application commands
                cmd = self.rx.recv() => match cmd {
                    Some(cmd) => {
                        if let Err(e) = self.process(cmd).await {
                            tracing::error!("Failed to process command: {}", e);
                            // Continue running even if command fails
                        }
                    },
                    None => break, // all senders dropped → shut down
                },

                // ❷ Detect a low-level close / socket error
                _ = self.ws.closed_notifier() => {
                    tracing::warn!("WebSocket connection lost, attempting reconnection");
                    if let Err(e) = self.reconnect_with_backoff(&mut backoff).await {
                        tracing::error!("Reconnection failed: {}", e);
                        break;
                    }
                    // Reset backoff on successful reconnection
                    backoff.reset();
                },

                // ❸ Heartbeat (ping every N seconds)
                _ = hb.tick() => {
                    if let Err(e) = self.ws.try_ping().await {
                        tracing::warn!("Ping failed: {}", e);
                        // Don't break on ping failure, let the closed_notifier handle it
                    }
                },

                // ❹ External shutdown signal
                _ = self.shutdown.cancelled() => {
                    tracing::info!("Shutdown signal received");
                    break;
                },
            }
        }
        Ok(())
    }

    async fn process(&self, cmd: Command) -> Result<()> {
        use Command::*;
        match cmd {
            Delete => self.ws.delete().await,
            SetAuthToken { auth_token } => self.ws.set_auth_token(&auth_token).await,
            // Add other command variants here as needed
            _ => Ok(()), // default no-op
        }
    }

    async fn reconnect_with_backoff(&self, backoff: &mut ExponentialBackoff) -> Result<()> {
        while let Some(delay) = backoff.next_backoff() {
            tracing::info!(
                "Attempting reconnection in {:?} (attempt {})",
                delay,
                backoff.attempts
            );
            sleep(delay).await;

            // Check for shutdown during backoff
            if self.shutdown.is_cancelled() {
                return Err(Error::Internal(
                    "Shutdown requested during reconnection".into(),
                ));
            }

            match self.ws.reconnect().await {
                Ok(_) => {
                    tracing::info!("WebSocket reconnected successfully");
                    return Ok(());
                }
                Err(e) => {
                    tracing::warn!("Reconnection attempt {} failed: {}", backoff.attempts, e);
                }
            }
        }
        Err(Error::Internal(
            "Reconnection backoff exhausted after maximum attempts".into(),
        ))
    }

    /// Get a handle to request shutdown
    pub fn shutdown_token(&self) -> CancellationToken {
        self.shutdown.clone()
    }
}
