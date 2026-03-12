//! Configuration settings and loading.

use std::env;
use dotenvy::dotenv;
use serde::Deserialize;
use anyhow::{Result, Context};

/// Application configuration.
#[derive(Debug, Clone)]
pub struct Config {
    /// Target symbols (comma-separated)
    pub symbols: Vec<String>,

    /// Time window interval in seconds
    pub window_interval: u64,

    /// EMA window length for statistics
    pub ema_window: usize,

    /// WebSocket reconnect interval in seconds
    pub ws_reconnect_interval: u64,

    /// WebSocket timeout in seconds
    pub ws_timeout: u64,

    /// Maximum WebSocket reconnect retries
    pub ws_max_retries: u32,

    /// UI refresh interval in seconds
    pub ui_refresh_interval: u64,

    /// Log output interval in seconds
    pub log_output_interval: u64,

    /// Metrics output interval in seconds
    pub metrics_output_interval: u64,

    /// Maximum queue size
    pub queue_max_size: usize,

    /// Queue warning threshold
    pub queue_warning_threshold: usize,

    /// Log level
    pub log_level: String,

    /// Log file path
    pub log_file: String,

    /// Maximum log file size
    pub log_max_size: String,

    /// Number of log backups to keep
    pub log_backup_count: u32,

    /// Performance test mode
    pub performance_test_mode: bool,

    /// Test symbol count
    pub test_symbol_count: usize,

    /// Test duration in hours
    pub test_duration_hours: u64,
}

impl Config {
    /// Load configuration from environment variables.
    pub fn load() -> Result<Self> {
        // Load .env file if it exists
        dotenv().ok();

        // Parse symbols
        let symbols_str = env::var("SYMBOLS").unwrap_or_else(|_| "SOLUSDT,BTCUSDT".to_string());
        let mut symbols: Vec<String> = symbols_str
            .split(',')
            .map(|s| s.trim().to_lowercase())
            .collect();

        // Check if performance test mode is enabled
        let performance_test_mode = env::var("PERFORMANCE_TEST_MODE")
            .unwrap_or_else(|_| "false".to_string())
            .to_lowercase() == "true";

        // Apply test mode if enabled
        if performance_test_mode {
            let test_count: usize = env::var("TEST_SYMBOL_COUNT")
                .unwrap_or_else(|_| "50".to_string())
                .parse()
                .context("Failed to parse TEST_SYMBOL_COUNT")?;

            if symbols.len() < test_count {
                let base_symbols = symbols.clone();
                let repeat_count = (test_count / base_symbols.len()) + 1;
                symbols = base_symbols
                    .into_iter()
                    .cycle()
                    .take(test_count)
                    .collect();
            }
        }

        Ok(Config {
            symbols,
            window_interval: env::var("WINDOW_INTERVAL")
                .unwrap_or_else(|_| "60".to_string())
                .parse()
                .context("Failed to parse WINDOW_INTERVAL")?,
            ema_window: env::var("EMA_WINDOW")
                .unwrap_or_else(|_| "30".to_string())
                .parse()
                .context("Failed to parse EMA_WINDOW")?,
            ws_reconnect_interval: env::var("WS_RECONNECT_INTERVAL")
                .unwrap_or_else(|_| "5".to_string())
                .parse()
                .context("Failed to parse WS_RECONNECT_INTERVAL")?,
            ws_timeout: env::var("WS_TIMEOUT")
                .unwrap_or_else(|_| "30".to_string())
                .parse()
                .context("Failed to parse WS_TIMEOUT")?,
            ws_max_retries: env::var("WS_MAX_RETRIES")
                .unwrap_or_else(|_| "10".to_string())
                .parse()
                .context("Failed to parse WS_MAX_RETRIES")?,
            ui_refresh_interval: env::var("UI_REFRESH_INTERVAL")
                .unwrap_or_else(|_| "2".to_string())
                .parse()
                .context("Failed to parse UI_REFRESH_INTERVAL")?,
            log_output_interval: env::var("LOG_OUTPUT_INTERVAL")
                .unwrap_or_else(|_| "10".to_string())
                .parse()
                .context("Failed to parse LOG_OUTPUT_INTERVAL")?,
            metrics_output_interval: env::var("METRICS_OUTPUT_INTERVAL")
                .unwrap_or_else(|_| "60".to_string())
                .parse()
                .context("Failed to parse METRICS_OUTPUT_INTERVAL")?,
            queue_max_size: env::var("QUEUE_MAX_SIZE")
                .unwrap_or_else(|_| "10000".to_string())
                .parse()
                .context("Failed to parse QUEUE_MAX_SIZE")?,
            queue_warning_threshold: env::var("QUEUE_WARNING_THRESHOLD")
                .unwrap_or_else(|_| "8000".to_string())
                .parse()
                .context("Failed to parse QUEUE_WARNING_THRESHOLD")?,
            log_level: env::var("LOG_LEVEL")
                .unwrap_or_else(|_| "INFO".to_string()),
            log_file: env::var("LOG_FILE")
                .unwrap_or_else(|_| "logs/rust-basis.log".to_string()),
            log_max_size: env::var("LOG_MAX_SIZE")
                .unwrap_or_else(|_| "100MB".to_string()),
            log_backup_count: env::var("LOG_BACKUP_COUNT")
                .unwrap_or_else(|_| "10".to_string())
                .parse()
                .context("Failed to parse LOG_BACKUP_COUNT")?,
            performance_test_mode,
            test_symbol_count: env::var("TEST_SYMBOL_COUNT")
                .unwrap_or_else(|_| "50".to_string())
                .parse()
                .context("Failed to parse TEST_SYMBOL_COUNT")?,
            test_duration_hours: env::var("TEST_DURATION_HOURS")
                .unwrap_or_else(|_| "24".to_string())
                .parse()
                .context("Failed to parse TEST_DURATION_HOURS")?,
        })
    }

    /// Max streams per combined WebSocket connection (Binance limit ~1024, we use 120 for stability)
    const MAX_STREAMS_PER_CONNECTION: usize = 120;

    /// Get WebSocket URL for spot market (single symbol, legacy).
    pub fn get_spot_ws_url(&self, symbol: &str) -> String {
        format!("wss://stream.binance.com:9443/ws/{}@bookTicker", symbol)
    }

    /// Get WebSocket URL for futures market (single symbol, legacy).
    pub fn get_futures_ws_url(&self, symbol: &str) -> String {
        format!("wss://fstream.binance.com/ws/{}@bookTicker", symbol)
    }

    /// Get combined stream URLs for spot (chunked by MAX_STREAMS_PER_CONNECTION).
    pub fn get_spot_combined_ws_urls(&self) -> Vec<String> {
        self.symbols
            .chunks(Self::MAX_STREAMS_PER_CONNECTION)
            .map(|chunk| {
                let streams = chunk
                    .iter()
                    .map(|s| format!("{}@bookTicker", s))
                    .collect::<Vec<_>>()
                    .join("/");
                format!("wss://stream.binance.com:9443/stream?streams={}", streams)
            })
            .collect()
    }

    /// Get combined stream URLs for futures (chunked by MAX_STREAMS_PER_CONNECTION).
    pub fn get_futures_combined_ws_urls(&self) -> Vec<String> {
        self.symbols
            .chunks(Self::MAX_STREAMS_PER_CONNECTION)
            .map(|chunk| {
                let streams = chunk
                    .iter()
                    .map(|s| format!("{}@bookTicker", s))
                    .collect::<Vec<_>>()
                    .join("/");
                format!("wss://fstream.binance.com/stream?streams={}", streams)
            })
            .collect()
    }

    /// Get the number of symbols being monitored.
    pub fn symbol_count(&self) -> usize {
        self.symbols.len()
    }
}