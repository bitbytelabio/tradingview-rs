use crate::{
    ChartHistoricalData, DataPoint, Error, Interval, MarketSymbol, MarketTicker, OHLCV as _,
    Result, SymbolInfo,
    callback::EventCallback,
    chart::ChartOptions,
    error::TradingViewError,
    history::{
        CompletionChannel, DataChannel, DataChannels, InfoChannel, REMAINING_DATA_TIMEOUT,
        resolve_auth_token,
    },
    options::Range,
    socket::DataServer,
    websocket::{SeriesInfo, WebSocket, WebSocketClient},
};
use bon::builder;
use serde_json::Value;
use std::sync::Arc;
use tokio::{
    spawn,
    sync::{Mutex, mpsc},
    time::timeout,
};

/// Fetch historical chart data from TradingView
#[builder]
pub async fn retrieve(
    auth_token: Option<&str>,
    ticker: Option<&MarketTicker>,
    symbol: Option<&str>,
    exchange: Option<&str>,
    interval: Interval,
    range: Option<Range>,
    server: Option<DataServer>,
    num_bars: Option<u64>,
    #[builder(default = false)] with_replay: bool,
) -> Result<ChartHistoricalData> {
    let auth_token = resolve_auth_token(auth_token)?;
    let channels = DataChannels::new();

    let range = range.map(String::from);

    let (symbol, exchange) = if let Some(ticker) = ticker {
        (ticker.symbol().to_string(), ticker.exchange().to_string())
    } else if let Some(symbol) = symbol {
        if let Some(exchange) = exchange {
            (symbol.to_string(), exchange.to_string())
        } else {
            return Err(Error::TradingViewError(TradingViewError::MissingExchange));
        }
    } else {
        return Err(Error::TradingViewError(TradingViewError::MissingSymbol));
    };

    let options = ChartOptions::builder()
        .symbol(symbol)
        .exchange(exchange)
        .interval(interval)
        .maybe_range(range)
        .maybe_bar_count(num_bars)
        .replay_mode(with_replay)
        .build();

    // Create and configure WebSocket connection
    let callbacks = create_callbacks(&channels, with_replay);
    let websocket = setup_connection(&auth_token, server, callbacks, options).await?;
    let websocket = Arc::new(websocket);

    // Start subscription in background
    let subscription_handle = start_subscription_task(Arc::clone(&websocket));

    // Collect data with proper error handling
    let result = collect_historical_data(&channels, &websocket, with_replay).await;

    // Cleanup
    cleanup_resources(websocket, subscription_handle).await;

    match result {
        Ok(data) => {
            tracing::debug!("Data collection completed with {} points", data.data.len());
            Ok(data)
        }
        Err(e) => {
            tracing::error!("Failed to collect historical data: {}", e);
            Err(e)
        }
    }
}

/// Create callbacks for single symbol data fetching
fn create_callbacks(channels: &DataChannels, with_replay: bool) -> EventCallback {
    let data_tx = Arc::clone(&channels.data_tx);
    let info_tx = Arc::clone(&channels.info_tx);
    let completion_tx = Arc::clone(&channels.completion_tx);

    EventCallback::default()
        .on_chart_data(create_data_handler(data_tx))
        .on_symbol_info(create_symbol_handler(info_tx))
        .on_series_completed(create_completion_handler(completion_tx, with_replay))
        .on_error(create_error_handler(Arc::clone(&channels.completion_tx)))
}

/// Create chart data handler
fn create_data_handler(
    data_tx: Arc<Mutex<DataChannel>>,
) -> impl Fn((SeriesInfo, Vec<DataPoint>)) + Send + Sync + 'static {
    move |(series_info, data_points): (SeriesInfo, Vec<DataPoint>)| {
        tracing::debug!("Received data batch with {} points", data_points.len());
        let sender = Arc::clone(&data_tx);
        spawn(async move {
            if let Ok(tx) = sender.try_lock() {
                if let Err(e) = tx.send((series_info, data_points)).await {
                    tracing::error!("Failed to send data points: {}", e);
                }
            } else {
                tracing::warn!("Data channel is locked, skipping batch");
            }
        });
    }
}

/// Create symbol info handler
fn create_symbol_handler(
    info_tx: Arc<Mutex<InfoChannel>>,
) -> impl Fn(SymbolInfo) + Send + Sync + 'static {
    move |symbol_info| {
        tracing::debug!("Received symbol info: {:?}", symbol_info);
        let sender = Arc::clone(&info_tx);
        spawn(async move {
            if let Ok(tx) = sender.try_lock() {
                if let Err(e) = tx.send(symbol_info).await {
                    tracing::error!("Failed to send symbol info: {}", e);
                }
            } else {
                tracing::warn!("Info channel is locked, skipping symbol info");
            }
        });
    }
}

/// Create series completion handler
fn create_completion_handler(
    completion_tx: Arc<Mutex<Option<CompletionChannel>>>,
    with_replay: bool,
) -> impl Fn(Vec<Value>) + Send + Sync + 'static {
    move |message: Vec<Value>| {
        let completion_tx = Arc::clone(&completion_tx);
        let should_complete = if with_replay {
            let message_json = serde_json::to_string(&message).unwrap_or_default();
            message_json.contains("replay") && message_json.contains("data_completed")
        } else {
            true
        };

        if should_complete {
            spawn(async move {
                send_completion_signal(completion_tx, "Series completed").await;
            });
        }
    }
}

/// Create error handler
fn create_error_handler(
    completion_tx: Arc<Mutex<Option<CompletionChannel>>>,
) -> impl Fn((Error, Vec<Value>)) + Send + Sync + 'static {
    move |(error, message)| {
        tracing::error!("WebSocket error: {:?}", message);

        let is_critical = matches!(
            error,
            Error::LoginError(_)
                | Error::NoChartTokenFound
                | Error::WebSocketError(_)
                | Error::TradingViewError(
                    TradingViewError::CriticalError | TradingViewError::ProtocolError
                )
        );

        if is_critical {
            let completion_tx = Arc::clone(&completion_tx);
            spawn(async move {
                send_completion_signal(completion_tx, "Critical error occurred").await;
            });
        }
    }
}

/// Send completion signal safely
async fn send_completion_signal(
    completion_tx: Arc<Mutex<Option<CompletionChannel>>>,
    reason: &str,
) {
    if let Ok(mut tx_opt) = completion_tx.try_lock() {
        if let Some(tx) = tx_opt.take() {
            tracing::debug!("{}, sending completion signal", reason);
            if tx.send(()).is_err() {
                tracing::warn!("Completion receiver was dropped");
            }
        }
    }
}

/// Set up WebSocket connection
async fn setup_connection(
    auth_token: &str,
    server: Option<DataServer>,
    callbacks: EventCallback,
    options: ChartOptions,
) -> Result<WebSocket> {
    let client = WebSocketClient::default().set_callbacks(callbacks);

    let websocket = WebSocket::new()
        .server(server.unwrap_or(DataServer::ProData))
        .auth_token(auth_token)
        .client(client)
        .build()
        .await?;

    websocket.set_market(options).await?;
    Ok(websocket)
}

/// Start subscription task
fn start_subscription_task(websocket: Arc<WebSocket>) -> tokio::task::JoinHandle<()> {
    spawn(async move {
        websocket.subscribe().await;
    })
}

/// Collect historical data from channels
async fn collect_historical_data(
    channels: &DataChannels,
    websocket: &Arc<WebSocket>,
    with_replay: bool,
) -> Result<ChartHistoricalData> {
    let mut historical_data = ChartHistoricalData::new();
    let mut completion_rx = channels.completion_rx.lock().await;
    let mut data_rx = channels.data_rx.lock().await;
    let mut info_rx = channels.info_rx.lock().await;
    let mut is_replayed = false;

    loop {
        tokio::select! {
            Some((series_info, data_points)) = data_rx.recv() => {
                handle_data(&mut historical_data, series_info, data_points,
                    websocket, with_replay, &mut is_replayed).await?;
            }
            Some(symbol_info) = info_rx.recv() => {
                tracing::debug!("Processing symbol info: {:?}", symbol_info);
                historical_data.symbol_info = symbol_info;
            }
            _ = &mut *completion_rx => {
                tracing::debug!("Completion signal received");
                process_remaining_data(&mut data_rx, &mut info_rx, &mut historical_data).await;
                break;
            }
            else => {
                tracing::debug!("All channels closed");
                break;
            }
        }
    }

    Ok(historical_data)
}

/// Handle incoming data batch
async fn handle_data(
    historical_data: &mut ChartHistoricalData,
    series_info: SeriesInfo,
    data_points: Vec<DataPoint>,
    websocket: &Arc<WebSocket>,
    with_replay: bool,
    is_replayed: &mut bool,
) -> Result<()> {
    tracing::debug!("Processing batch of {} data points", data_points.len());

    historical_data.series_info = series_info;
    historical_data.data.extend(data_points);

    // Handle replay mode setup
    if with_replay && !historical_data.data.is_empty() && !*is_replayed {
        setup_replay_mode(historical_data, websocket).await?;
        *is_replayed = true;
    }

    Ok(())
}

/// Setup replay mode
async fn setup_replay_mode(
    historical_data: &mut ChartHistoricalData,
    websocket: &Arc<WebSocket>,
) -> Result<()> {
    historical_data.data.sort_by_key(|point| point.timestamp());

    let earliest_timestamp = historical_data
        .data
        .first()
        .map(|point| point.timestamp())
        .unwrap_or(0);

    let mut options = historical_data.series_info.options.clone();
    options.replay_mode = true;
    options.replay_from = earliest_timestamp;

    tracing::debug!("Setting replay mode with timestamp: {}", earliest_timestamp);
    websocket.set_market(options).await?;

    Ok(())
}

/// Process remaining data in channels
async fn process_remaining_data(
    data_rx: &mut mpsc::Receiver<(SeriesInfo, Vec<DataPoint>)>,
    info_rx: &mut mpsc::Receiver<SymbolInfo>,
    historical_data: &mut ChartHistoricalData,
) {
    // Process remaining data points
    while let Ok(Some((series_info, data_points))) =
        timeout(REMAINING_DATA_TIMEOUT, data_rx.recv()).await
    {
        tracing::debug!(
            "Processing final batch of {} data points",
            data_points.len()
        );
        historical_data.series_info = series_info;
        historical_data.data.extend(data_points);
    }

    // Process remaining symbol info
    while let Ok(Some(symbol_info)) = timeout(REMAINING_DATA_TIMEOUT, info_rx.recv()).await {
        tracing::debug!("Processing final symbol info: {:?}", symbol_info);
        historical_data.symbol_info = symbol_info;
    }
}

/// Cleanup resources
async fn cleanup_resources(
    websocket: Arc<WebSocket>,
    subscription_handle: tokio::task::JoinHandle<()>,
) {
    if let Err(e) = websocket.delete().await {
        tracing::error!("Failed to close WebSocket connection: {}", e);
    }
    subscription_handle.abort();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Interval, MarketTicker};
    use std::sync::Once;

    fn init() {
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            tracing_subscriber::fmt()
                .with_max_level(tracing::Level::DEBUG)
                .init();
        });
    }

    #[tokio::test]
    async fn test_fetch_chart_historical() -> anyhow::Result<()> {
        init();
        dotenv::dotenv().ok();

        let auth_token = std::env::var("TV_AUTH_TOKEN").expect("TV_AUTH_TOKEN is not set");

        let ticker = MarketTicker::new("VCB", "HOSE");
        let data = retrieve()
            .auth_token(&auth_token)
            .ticker(&ticker)
            .interval(Interval::OneDay)
            .server(DataServer::ProData)
            .num_bars(10)
            .call()
            .await?;

        assert!(!data.data.is_empty(), "Data should not be empty");
        assert_eq!(data.data.len(), 10, "Data length should be 10");
        assert_eq!(data.symbol_info.id, "HOSE:VCB", "Symbol should match");

        Ok(())
    }
}
