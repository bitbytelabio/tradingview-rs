use crate::{DataPoint, Error, Result, SymbolInfo, error::LoginError, websocket::SeriesInfo};
use std::{sync::Arc, time::Duration};
use tokio::sync::{Mutex, mpsc, oneshot};

pub mod batch;
pub mod single;

// Type aliases for better readability
type DataChannel = mpsc::Sender<(SeriesInfo, Vec<DataPoint>)>;
type InfoChannel = mpsc::Sender<SymbolInfo>;
type CompletionChannel = oneshot::Sender<()>;

// Configuration constants
const DATA_CHANNEL_BUFFER: usize = 2000;
const INFO_CHANNEL_BUFFER: usize = 100;
const REMAINING_DATA_TIMEOUT: Duration = Duration::from_millis(100);

/// Handles for managing data communication channels
#[allow(clippy::type_complexity)]
#[derive(Debug)]
struct DataChannels {
    data_tx: Arc<Mutex<DataChannel>>,
    info_tx: Arc<Mutex<InfoChannel>>,
    completion_tx: Arc<Mutex<Option<CompletionChannel>>>,
    data_rx: Arc<Mutex<mpsc::Receiver<(SeriesInfo, Vec<DataPoint>)>>>,
    info_rx: Arc<Mutex<mpsc::Receiver<SymbolInfo>>>,
    completion_rx: Arc<Mutex<oneshot::Receiver<()>>>,
}

impl DataChannels {
    fn new() -> Self {
        let (data_tx, data_rx) = mpsc::channel(DATA_CHANNEL_BUFFER);
        let (info_tx, info_rx) = mpsc::channel(INFO_CHANNEL_BUFFER);
        let (completion_tx, completion_rx) = oneshot::channel();

        Self {
            data_tx: Arc::new(Mutex::new(data_tx)),
            info_tx: Arc::new(Mutex::new(info_tx)),
            completion_tx: Arc::new(Mutex::new(Some(completion_tx))),
            data_rx: Arc::new(Mutex::new(data_rx)),
            info_rx: Arc::new(Mutex::new(info_rx)),
            completion_rx: Arc::new(Mutex::new(completion_rx)),
        }
    }
}

/// Resolve authentication token from parameter or environment
fn resolve_auth_token(auth_token: Option<&str>) -> Result<String> {
    match auth_token {
        Some(token) => Ok(token.to_string()),
        None => {
            tracing::warn!("No auth token provided, using environment variable");
            std::env::var("TV_AUTH_TOKEN").map_err(|_| {
                tracing::error!("TV_AUTH_TOKEN environment variable is not set");
                Error::LoginError(LoginError::InvalidSession)
            })
        }
    }
}
