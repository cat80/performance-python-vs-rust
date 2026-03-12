//! WebSocket client for Binance markets.

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::net::TcpStream;
use tokio_tungstenite::{
    connect_async, tungstenite::protocol::Message, MaybeTlsStream, WebSocketStream,
};
use futures_util::StreamExt;
use serde_json::Value;
use anyhow::{Result, Context};
use tracing::{info, warn, error};
use crossbeam_channel::Sender;
use atomic_counter::AtomicCounter;

use crate::config::Config;

/// WebSocket message with metadata.
#[derive(Debug, Clone)]
pub struct WebSocketMessage {
    /// Raw message data
    pub data: Value,
    /// Market type: "spot" or "futures"
    pub market_type: String,
    /// Trading symbol
    pub symbol: String,
    /// Timestamp when message was received (Unix secs)
    pub received_timestamp: u64,
    /// When message was put in queue (for latency calc, millis since epoch)
    pub queue_entry_millis: u64,
}

/// WebSocket client for Binance markets.
pub struct WebSocketClient {
    /// Application configuration
    config: Config,
    /// Trading symbol
    symbol: String,
    /// Market type: "spot" or "futures"
    market_type: String,
    /// Channel sender for processed data
    tx: Sender<WebSocketMessage>,
    /// Received counter for receive_rate (increment when sending)
    received_counter: Option<Arc<atomic_counter::RelaxedCounter>>,
    /// WebSocket URL
    url: String,
    /// Reconnect attempt count
    reconnect_count: u32,
    /// Running flag
    running: bool,
}

impl WebSocketClient {
    /// Create a new WebSocket client.
    pub fn new(
        config: Config,
        symbol: String,
        tx: Sender<WebSocketMessage>,
        received_counter: Option<Arc<atomic_counter::RelaxedCounter>>,
        market_type: String,
    ) -> Self {
        let url = if market_type == "spot" {
            config.get_spot_ws_url(&symbol)
        } else {
            config.get_futures_ws_url(&symbol)
        };

        Self {
            config,
            symbol,
            market_type,
            tx,
            received_counter,
            url,
            reconnect_count: 0,
            running: true,
        }
    }

    /// Connect to WebSocket and start listening.
    pub async fn connect(&mut self) -> Result<()> {
        while self.running && self.reconnect_count < self.config.ws_max_retries {
            info!(
                "Connecting to {} WebSocket for {}: {}",
                self.market_type,
                self.symbol.to_uppercase(),
                self.url
            );

            match self.try_connect().await {
                Ok(()) => {
                    self.reconnect_count = 0;
                    info!(
                        "{} WebSocket connected for {}",
                        self.market_type,
                        self.symbol.to_uppercase()
                    );
                }
                Err(e) => {
                    error!(
                        "{} WebSocket error for {}: {}",
                        self.market_type,
                        self.symbol.to_uppercase(),
                        e
                    );

                    if self.running {
                        self.reconnect_count += 1;
                        let wait_time =
                            Duration::from_secs(self.config.ws_reconnect_interval * self.reconnect_count as u64);

                        info!(
                            "Reconnecting {} for {} in {}s (attempt {})",
                            self.market_type,
                            self.symbol.to_uppercase(),
                            wait_time.as_secs(),
                            self.reconnect_count
                        );

                        tokio::time::sleep(wait_time).await;
                    }
                }
            }
        }

        Ok(())
    }

    /// Try to connect and listen for messages.
    async fn try_connect(&self) -> Result<()> {
        let (ws_stream, _) = connect_async(&self.url).await
            .with_context(|| format!("Failed to connect to {}", self.url))?;

        self.listen(ws_stream).await
    }

    /// Listen for messages from WebSocket.
    async fn listen(&self, mut ws_stream: WebSocketStream<MaybeTlsStream<TcpStream>>) -> Result<()> {
        while self.running {
            match ws_stream.next().await {
                Some(Ok(message)) => {
                    if let Message::Text(text) = message {
                        self.process_message(&text).await?;
                    }
                }
                Some(Err(e)) => {
                    error!("WebSocket error: {}", e);
                    break;
                }
                None => {
                    warn!("WebSocket connection closed");
                    break;
                }
            }
        }

        Ok(())
    }

    /// Process a WebSocket message.
    async fn process_message(&self, text: &str) -> Result<()> {
        // Parse JSON message
        let data: Value = serde_json::from_str(text)
            .with_context(|| format!("Failed to parse JSON: {}", text))?;

        // Timestamps for receive_rate and latency
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
        let received_timestamp = now.as_secs();
        let queue_entry_millis = now.as_millis() as u64;

        // Create WebSocket message
        let message = WebSocketMessage {
            data,
            market_type: self.market_type.clone(),
            symbol: self.symbol.clone(),
            received_timestamp,
            queue_entry_millis,
        };

        // Send to channel (non-blocking)
        if self.tx.send(message).is_err() {
            warn!(
                "Channel closed, dropping {} message for {}",
                self.market_type,
                self.symbol.to_uppercase()
            );
        } else if let Some(ref c) = self.received_counter {
            c.inc();
        }

        Ok(())
    }

    /// Stop the WebSocket client.
    pub fn stop(&mut self) {
        self.running = false;
        info!(
            "Stopping {} WebSocket client for {}",
            self.market_type,
            self.symbol.to_uppercase()
        );
    }
}

/// Start a WebSocket client for the given symbol and market type (legacy, single-symbol).
pub async fn start_websocket_client(
    config: Config,
    symbol: String,
    tx: Sender<WebSocketMessage>,
    received_counter: Option<Arc<atomic_counter::RelaxedCounter>>,
    market_type: String,
) -> Result<()> {
    let mut client = WebSocketClient::new(config, symbol, tx, received_counter, market_type);
    client.connect().await
}

/// Combined stream message format: {"stream":"btcusdt@bookTicker","data":{...}}
#[derive(serde::Deserialize)]
struct CombinedStreamMessage {
    stream: Option<String>,
    data: Value,
}

/// Start a combined WebSocket client (multiple symbols in one connection, max 120 per conn).
pub async fn start_combined_websocket_client(
    url: String,
    market_type: String,
    tx: Sender<WebSocketMessage>,
    received_counter: Option<Arc<atomic_counter::RelaxedCounter>>,
) -> Result<()> {
    let mut reconnect_count = 0u32;
    let max_retries = 10u32;

    while reconnect_count < max_retries {
        info!("Connecting to {} combined WebSocket", market_type);

        match try_connect_combined(&url, &market_type, &tx, &received_counter).await {
            Ok(()) => {
                reconnect_count = 0;
                info!("{} combined WebSocket connected", market_type);
            }
            Err(e) => {
                error!("{} combined WebSocket error: {}", market_type, e);
                reconnect_count += 1;
                let wait = Duration::from_secs(reconnect_count as u64);
                info!("Reconnecting {} in {}s (attempt {})", market_type, wait.as_secs(), reconnect_count);
                tokio::time::sleep(wait).await;
            }
        }
    }

    Ok(())
}

async fn try_connect_combined(
    url: &str,
    market_type: &str,
    tx: &Sender<WebSocketMessage>,
    received_counter: &Option<Arc<atomic_counter::RelaxedCounter>>,
) -> Result<()> {
    let (ws_stream, _) = connect_async(url)
        .await
        .with_context(|| format!("Failed to connect to {}", url))?;

    listen_combined(ws_stream, market_type, tx, received_counter).await
}

async fn listen_combined(
    mut ws_stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
    market_type: &str,
    tx: &Sender<WebSocketMessage>,
    received_counter: &Option<Arc<atomic_counter::RelaxedCounter>>,
) -> Result<()> {
    while let Some(msg) = ws_stream.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                if let Err(e) = process_combined_message(&text, market_type, tx, received_counter).await {
                    warn!("Failed to process combined message: {}", e);
                }
            }
            Ok(_) => {}
            Err(e) => {
                error!("WebSocket error: {}", e);
                break;
            }
        }
    }
    Ok(())
}

async fn process_combined_message(
    text: &str,
    market_type: &str,
    tx: &Sender<WebSocketMessage>,
    received_counter: &Option<Arc<atomic_counter::RelaxedCounter>>,
) -> Result<()> {
    let wrapper: CombinedStreamMessage = serde_json::from_str(text)
        .with_context(|| format!("Failed to parse: {}", &text[..text.len().min(100)]))?;

    let mut data = wrapper.data;
    // Extract symbol from stream "btcusdt@bookTicker" or data["s"]
    let symbol = wrapper
        .stream
        .as_ref()
        .and_then(|s| s.split('@').next())
        .or_else(|| data["s"].as_str())
        .unwrap_or("unknown")
        .to_string()
        .to_lowercase();

    // Normalize b/a -> bestBidPrice/bestAskPrice (for handler compatibility)
    if let Some(b) = data.get("b").cloned() {
        data["bestBidPrice"] = b;
    }
    if let Some(a) = data.get("a").cloned() {
        data["bestAskPrice"] = a;
    }

    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    let message = WebSocketMessage {
        data,
        market_type: market_type.to_string(),
        symbol,
        received_timestamp: now.as_secs(),
        queue_entry_millis: now.as_millis() as u64,
    };

    if tx.send(message).is_err() {
        warn!("Channel closed, dropping {} message", market_type);
    } else if let Some(c) = received_counter {
        c.inc();
    }

    Ok(())
}