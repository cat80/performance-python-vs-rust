//! Basis calculation for spot and futures prices.

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use serde_json::Value;
use tracing::{debug, error};
use anyhow::{Result, Context};

/// Data for basis calculation results.
#[derive(Debug, Clone)]
pub struct BasisData {
    /// Trading symbol
    pub symbol: String,
    /// Timestamp
    pub timestamp: u64,
    /// Spot mid price
    pub spot_price: f64,
    /// Futures mid price
    pub futures_price: f64,
    /// Basis: (future - spot) / spot
    pub basis: f64,
    /// Spot mid price (alias)
    pub spot_mid: f64,
    /// Futures mid price (alias)
    pub futures_mid: f64,
}

/// Calculator for basis between spot and futures prices.
pub struct BasisCalculator {
    /// Time window interval in seconds
    window_interval: u64,
    /// Basis history by symbol
    basis_history: HashMap<String, Vec<BasisData>>,
    /// Current window data by symbol
    window_data: HashMap<String, Vec<BasisData>>,
    /// Window start time by symbol
    window_start_time: HashMap<String, u64>,
}

impl BasisCalculator {
    /// Create a new basis calculator.
    pub fn new(window_interval: u64) -> Self {
        Self {
            window_interval,
            basis_history: HashMap::new(),
            window_data: HashMap::new(),
            window_start_time: HashMap::new(),
        }
    }

    /// Calculate basis from spot and futures data.
    pub fn calculate_basis(
        &mut self,
        spot_data: &Value,
        futures_data: &Value,
        symbol: &str,
    ) -> Result<Option<BasisData>> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Calculate mid prices
        let spot_mid = self.get_mid_price(spot_data)
            .with_context(|| format!("Failed to get spot mid price for {}", symbol))?;

        let futures_mid = self.get_mid_price(futures_data)
            .with_context(|| format!("Failed to get futures mid price for {}", symbol))?;

        // Calculate basis: (future - spot) / spot
        let basis = if spot_mid > 0.0 {
            (futures_mid - spot_mid) / spot_mid
        } else {
            error!("Zero spot price for {}", symbol);
            return Ok(None);
        };

        // Create basis data
        let basis_data = BasisData {
            symbol: symbol.to_uppercase(),
            timestamp,
            spot_price: spot_mid,
            futures_price: futures_mid,
            basis,
            spot_mid,
            futures_mid,
        };

        // Add to window
        self.add_to_window(&basis_data);

        Ok(Some(basis_data))
    }

    /// Get mid price from bookTicker data.
    fn get_mid_price(&self, data: &Value) -> Result<f64> {
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

    /// Add basis data to current time window.
    fn add_to_window(&mut self, basis_data: &BasisData) {
        let symbol = basis_data.symbol.clone();

        // Initialize window if needed
        if !self.window_start_time.contains_key(&symbol) {
            self.window_start_time.insert(symbol.clone(), basis_data.timestamp);
        }

        // Check if window has expired
        let window_age = basis_data.timestamp - self.window_start_time[&symbol];
        if window_age >= self.window_interval {
            // Window expired, aggregate and move to history
            self.aggregate_window(&symbol);
            self.window_start_time.insert(symbol.clone(), basis_data.timestamp);
            self.window_data.remove(&symbol);
        }

        // Add to current window
        self.window_data
            .entry(symbol)
            .or_insert_with(Vec::new)
            .push(basis_data.clone());
    }

    /// Aggregate data in the current window and add to history.
    fn aggregate_window(&mut self, symbol: &str) {
        let Some(window_data) = self.window_data.get(symbol) else {
            return;
        };

        if window_data.is_empty() {
            return;
        }

        // Calculate window statistics
        let basis_values: Vec<f64> = window_data.iter().map(|d| d.basis).collect();
        let spot_prices: Vec<f64> = window_data.iter().map(|d| d.spot_price).collect();
        let futures_prices: Vec<f64> = window_data.iter().map(|d| d.futures_price).collect();

        let avg_basis: f64 = basis_values.iter().sum::<f64>() / basis_values.len() as f64;
        let avg_spot: f64 = spot_prices.iter().sum::<f64>() / spot_prices.len() as f64;
        let avg_futures: f64 = futures_prices.iter().sum::<f64>() / futures_prices.len() as f64;

        let window_start = self.window_start_time[symbol];
        let aggregated_data = BasisData {
            symbol: symbol.to_string(),
            timestamp: window_start + self.window_interval / 2,
            spot_price: avg_spot,
            futures_price: avg_futures,
            basis: avg_basis,
            spot_mid: avg_spot,
            futures_mid: avg_futures,
        };

        // Add to history
        self.basis_history
            .entry(symbol.to_string())
            .or_insert_with(Vec::new)
            .push(aggregated_data.clone());

        // Keep only last N windows in history (e.g., last 1000)
        let max_history = 1000;
        if let Some(history) = self.basis_history.get_mut(symbol) {
            if history.len() > max_history {
                *history = history[history.len() - max_history..].to_vec();
            }
        }

        debug!(
            "Aggregated window for {}: basis={:.6}%, samples={}",
            symbol,
            avg_basis * 100.0,
            basis_values.len()
        );
    }

    /// Get recent basis data for a symbol.
    pub fn get_recent_basis_data(&self, symbol: &str, count: usize) -> Vec<BasisData> {
        self.basis_history
            .get(&symbol.to_uppercase())
            .map(|history| {
                let start = if history.len() > count {
                    history.len() - count
                } else {
                    0
                };
                history[start..].to_vec()
            })
            .unwrap_or_default()
    }

    /// Get current window data for a symbol.
    pub fn get_current_window_data(&self, symbol: &str) -> Vec<BasisData> {
        self.window_data
            .get(&symbol.to_uppercase())
            .cloned()
            .unwrap_or_default()
    }

    /// Get statistics for current window.
    pub fn get_window_stats(&self, symbol: &str) -> WindowStats {
        let symbol_key = symbol.to_uppercase();
        let current_data = self.window_data.get(&symbol_key);

        if current_data.is_none() || current_data.unwrap().is_empty() {
            return WindowStats::default();
        }

        let data = current_data.unwrap();
        let basis_values: Vec<f64> = data.iter().map(|d| d.basis).collect();
        let current_basis = *basis_values.last().unwrap_or(&0.0);
        let avg_basis = basis_values.iter().sum::<f64>() / basis_values.len() as f64;

        // Calculate window progress
        let window_progress = if let Some(window_start) = self.window_start_time.get(&symbol_key) {
            let current_time = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let window_age = current_time - window_start;
            (window_age as f64 / self.window_interval as f64).min(1.0)
        } else {
            0.0
        };

        WindowStats {
            sample_count: data.len(),
            current_basis,
            avg_basis,
            window_progress,
            window_start: *self.window_start_time.get(&symbol_key).unwrap_or(&0),
            window_end: self.window_start_time.get(&symbol_key).unwrap_or(&0) + self.window_interval,
        }
    }
}

/// Statistics for a time window.
#[derive(Debug, Clone, Default)]
pub struct WindowStats {
    /// Number of samples in window
    pub sample_count: usize,
    /// Current basis value
    pub current_basis: f64,
    /// Average basis in window
    pub avg_basis: f64,
    /// Window progress (0.0 to 1.0)
    pub window_progress: f64,
    /// Window start time (seconds since epoch)
    pub window_start: u64,
    /// Window end time (seconds since epoch)
    pub window_end: u64,
}