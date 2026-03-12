//! Message handler for WebSocket data.

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use serde_json::Value;
use tracing::{debug, warn};
use anyhow::{Result, Context};

use super::client::WebSocketMessage;

/// Handler for processing WebSocket messages.
pub struct MessageHandler {
    /// Spot prices by symbol
    spot_prices: HashMap<String, WebSocketMessage>,
    /// Futures prices by symbol
    futures_prices: HashMap<String, WebSocketMessage>,
    /// Last match time by symbol
    last_match_time: HashMap<String, u64>,
}

impl MessageHandler {
    /// Create a new message handler.
    pub fn new() -> Self {
        Self {
            spot_prices: HashMap::new(),
            futures_prices: HashMap::new(),
            last_match_time: HashMap::new(),
        }
    }

    /// Process a WebSocket message and match spot/futures prices.
    pub fn process_message(
        &mut self,
        message: WebSocketMessage,
    ) -> Option<(WebSocketMessage, WebSocketMessage)> {
        let symbol = message.symbol.clone();
        let market_type = message.market_type.clone();

        // Store price based on market type
        if market_type == "spot" {
            self.spot_prices.insert(symbol.clone(), message);
        } else {
            self.futures_prices.insert(symbol.clone(), message);
        }

        // Check if we have both spot and futures prices
        if let (Some(spot_data), Some(futures_data)) = (
            self.spot_prices.get(&symbol),
            self.futures_prices.get(&symbol),
        ) {
            // Calculate time difference
            let spot_time = spot_data.received_timestamp;
            let futures_time = futures_data.received_timestamp;
            let time_diff = (spot_time as i64 - futures_time as i64).abs() as u64;

            // Only process if prices are reasonably close in time (within 1 second)
            if time_diff < 1 {
                // Clone data before removing from maps
                let spot_data = spot_data.clone();
                let futures_data = futures_data.clone();

                // Clear stored prices to avoid reusing old data
                self.spot_prices.remove(&symbol);
                self.futures_prices.remove(&symbol);

                // Update last match time
                self.last_match_time.insert(
                    symbol.clone(),
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                );

                return Some((spot_data, futures_data));
            } else if time_diff > 5 {
                // Prices are too stale, clear them
                warn!(
                    "Price mismatch for {}: time diff {}s",
                    symbol.to_uppercase(),
                    time_diff
                );
                self.spot_prices.remove(&symbol);
                self.futures_prices.remove(&symbol);
            }
        }

        None
    }

    /// Calculate mid price from bookTicker data.
    pub fn get_mid_price(&self, data: &Value) -> Result<f64> {
        let best_bid_price = data["bestBidPrice"]
            .as_str()
            .context("Missing bestBidPrice")?
            .parse::<f64>()
            .context("Failed to parse bestBidPrice")?;

        let best_ask_price = data["bestAskPrice"]
            .as_str()
            .context("Missing bestAskPrice")?
            .parse::<f64>()
            .context("Failed to parse bestAskPrice")?;

        Ok((best_bid_price + best_ask_price) / 2.0)
    }

    /// Clean up stale price data.
    pub fn cleanup_stale_prices(&mut self, max_age: u64) {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Clean spot prices
        let stale_symbols: Vec<String> = self
            .spot_prices
            .iter()
            .filter(|(_, data)| current_time - data.received_timestamp > max_age)
            .map(|(symbol, _)| symbol.clone())
            .collect();

        for symbol in stale_symbols {
            self.spot_prices.remove(&symbol);
            debug!("Cleaned stale spot price for {}", symbol.to_uppercase());
        }

        // Clean futures prices
        let stale_symbols: Vec<String> = self
            .futures_prices
            .iter()
            .filter(|(_, data)| current_time - data.received_timestamp > max_age)
            .map(|(symbol, _)| symbol.clone())
            .collect();

        for symbol in stale_symbols {
            self.futures_prices.remove(&symbol);
            debug!("Cleaned stale futures price for {}", symbol.to_uppercase());
        }
    }

    /// Get the number of pending spot prices.
    pub fn pending_spot_count(&self) -> usize {
        self.spot_prices.len()
    }

    /// Get the number of pending futures prices.
    pub fn pending_futures_count(&self) -> usize {
        self.futures_prices.len()
    }
}

impl Default for MessageHandler {
    fn default() -> Self {
        Self::new()
    }
}