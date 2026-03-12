//! Statistical indicators calculation (MA, EMA, Z-Score).

use std::collections::HashMap;
use polars::prelude::*;
use tracing::{error, warn};
use anyhow::{Result, Context};

/// Result of indicator calculations.
#[derive(Debug, Clone)]
pub struct IndicatorResult {
    /// Trading symbol
    pub symbol: String,
    /// Timestamp
    pub timestamp: u64,
    /// Moving average of price
    pub ma_price: f64,
    /// Moving average of basis
    pub ma_basis: f64,
    /// Exponential moving average of basis
    pub ema_basis: f64,
    /// Z-Score of basis
    pub z_score: f64,
}

/// Calculator for statistical indicators.
pub struct IndicatorCalculator {
    /// Window size for MA/EMA calculations
    window_size: usize,
    /// Basis history by symbol (using native Vec for performance)
    basis_history: HashMap<String, Vec<f64>>,
    /// Price history by symbol (using native Vec for performance)
    price_history: HashMap<String, Vec<f64>>,
    /// EMA smoothing factor
    alpha: f64,
}

impl IndicatorCalculator {
    /// Create a new indicator calculator.
    pub fn new(window_size: usize) -> Self {
        Self {
            window_size,
            basis_history: HashMap::new(),
            price_history: HashMap::new(),
            alpha: 2.0 / (window_size as f64 + 1.0),
        }
    }

    /// Add basis and price data for a symbol.
    pub fn add_data(&mut self, symbol: &str, basis: f64, price: f64) {
        let symbol_key = symbol.to_uppercase();

        // Initialize vectors if needed
        self.basis_history
            .entry(symbol_key.clone())
            .or_insert_with(Vec::new)
            .push(basis);

        self.price_history
            .entry(symbol_key.clone())
            .or_insert_with(Vec::new)
            .push(price);

        // Keep only window_size most recent values
        if let Some(history) = self.basis_history.get_mut(&symbol_key) {
            if history.len() > self.window_size {
                *history = history[history.len() - self.window_size..].to_vec();
            }
        }

        if let Some(history) = self.price_history.get_mut(&symbol_key) {
            if history.len() > self.window_size {
                *history = history[history.len() - self.window_size..].to_vec();
            }
        }
    }

    /// Calculate indicators for a symbol.
    pub fn calculate_indicators(&self, symbol: &str) -> Result<Option<IndicatorResult>> {
        let symbol_key = symbol.to_uppercase();

        // Check if we have enough data
        let basis_data = match self.basis_history.get(&symbol_key) {
            Some(data) if data.len() >= self.window_size => data,
            _ => return Ok(None),
        };

        let price_data = match self.price_history.get(&symbol_key) {
            Some(data) if data.len() >= self.window_size => data,
            _ => return Ok(None),
        };

        // Use Polars for optimized calculations (convert only when needed)
        match self.calculate_with_polars(&symbol_key, basis_data, price_data) {
            Ok(result) => Ok(Some(result)),
            Err(e) => {
                warn!("Polars calculation failed for {}: {}, falling back to manual", symbol, e);
                // Fallback to manual calculation
                Ok(Some(self.calculate_manually(&symbol_key, basis_data, price_data)))
            }
        }
    }

    /// Calculate indicators using Polars (optimized).
    fn calculate_with_polars(
        &self,
        symbol: &str,
        basis_data: &[f64],
        price_data: &[f64],
    ) -> Result<IndicatorResult> {
        // Convert to Polars Series only when needed (performance optimization)
        let basis_series = Series::new("basis", basis_data);
        let price_series = Series::new("price", price_data);

        // Get the last window_size values
        let basis_window = basis_series.tail(Some(self.window_size));
        let price_window = price_series.tail(Some(self.window_size));

        // Calculate MA (simple moving average)
        let ma_basis = basis_window.mean().unwrap_or(0.0);
        let ma_price = price_window.mean().unwrap_or(0.0);

        // Calculate EMA (exponential moving average)
        let ema_basis = self.calculate_ema(basis_data);

        // Calculate Z-Score: (current - MA) / StdDev
        let current_basis = *basis_data.last().unwrap_or(&0.0);
        // Manual std: sqrt(variance) - use basis_data slice directly
        let n = basis_data.len().min(self.window_size);
        let start = basis_data.len().saturating_sub(n);
        let window: Vec<f64> = basis_data[start..].to_vec();
        let mean = ma_basis;
        let variance = if window.len() > 1 {
            window.iter().map(|x| (x - mean).powi(2)).sum::<f64>()
                / (window.len() - 1) as f64
        } else {
            0.0
        };
        let std_basis = variance.sqrt();

        let z_score = if std_basis > 0.0 {
            (current_basis - ma_basis) / std_basis
        } else {
            0.0
        };

        Ok(IndicatorResult {
            symbol: symbol.to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            ma_price,
            ma_basis,
            ema_basis,
            z_score,
        })
    }

    /// Calculate indicators manually (fallback).
    fn calculate_manually(
        &self,
        symbol: &str,
        basis_data: &[f64],
        price_data: &[f64],
    ) -> IndicatorResult {
        // Get the last window_size values
        let basis_window = &basis_data[basis_data.len() - self.window_size..];
        let price_window = &price_data[price_data.len() - self.window_size..];

        // Calculate MA (simple moving average)
        let ma_basis: f64 = basis_window.iter().sum::<f64>() / basis_window.len() as f64;
        let ma_price: f64 = price_window.iter().sum::<f64>() / price_window.len() as f64;

        // Calculate EMA (exponential moving average)
        let ema_basis = self.calculate_ema(basis_window);

        // Calculate Z-Score: (current - MA) / StdDev
        let current_basis = *basis_window.last().unwrap_or(&0.0);

        let variance: f64 = basis_window
            .iter()
            .map(|&x| (x - ma_basis).powi(2))
            .sum::<f64>()
            / basis_window.len() as f64;

        let std_basis = variance.sqrt();

        let z_score = if std_basis > 0.0 {
            (current_basis - ma_basis) / std_basis
        } else {
            0.0
        };

        IndicatorResult {
            symbol: symbol.to_string(),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            ma_price,
            ma_basis,
            ema_basis,
            z_score,
        }
    }

    /// Calculate EMA using the current alpha.
    fn calculate_ema(&self, data: &[f64]) -> f64 {
        if data.is_empty() {
            return 0.0;
        }

        // Use the last window_size values
        let window_data = if data.len() >= self.window_size {
            &data[data.len() - self.window_size..]
        } else {
            data
        };

        // Calculate EMA manually
        let mut ema = window_data[0];
        for &value in &window_data[1..] {
            ema = self.alpha * value + (1.0 - self.alpha) * ema;
        }

        ema
    }

    /// Calculate indicators for all symbols with sufficient data.
    pub fn calculate_all_indicators(&self) -> HashMap<String, IndicatorResult> {
        let mut results = HashMap::new();

        for symbol in self.basis_history.keys() {
            if let Ok(Some(result)) = self.calculate_indicators(symbol) {
                results.insert(symbol.clone(), result);
            }
        }

        results
    }

    /// Get the number of symbols with indicator data.
    pub fn symbol_count(&self) -> usize {
        self.basis_history.len()
    }

    /// Get the window size.
    pub fn window_size(&self) -> usize {
        self.window_size
    }
}

impl Default for IndicatorCalculator {
    fn default() -> Self {
        Self::new(30)
    }
}