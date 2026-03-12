//! Logging configuration for the basis monitor.

use std::fs;
use std::path::Path;
use tracing::{info, warn, error, Level};
use tracing_subscriber::{fmt, prelude::*, EnvFilter, Registry};
use tracing_appender::{non_blocking, rolling};
use anyhow::{Result, Context};

use crate::config::Config;
use crate::queue::QueueStats;

/// Setup logging for the application.
pub fn setup_logger(config: &Config) -> Result<()> {
    // Create log directory if it doesn't exist
    let log_dir = Path::new(&config.log_file)
        .parent()
        .unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(log_dir).context("Failed to create log directory")?;

    // File appender with rotation
    let file_appender = rolling::daily(log_dir, "rust-basis.log");
    let (non_blocking_file, _guard) = non_blocking(file_appender);

    // Console appender: show info for our crate (stderr, visible at startup)
    let console_filter = EnvFilter::new("rust_basis=info,warn");

    // Combined filter for file logging
    let file_filter = EnvFilter::new(&config.log_level.to_lowercase());

    // Configure subscriber (clone EnvFilter for each layer)
    let subscriber = Registry::default()
        .with(console_filter.clone())
        .with(
            fmt::Layer::new()
                .with_writer(std::io::stderr)
                .with_filter(console_filter.clone()),
        )
        .with(
            fmt::Layer::new()
                .with_writer(non_blocking_file)
                .with_ansi(false)
                .with_filter(file_filter),
        );

    tracing::subscriber::set_global_default(subscriber)
        .context("Failed to set global default subscriber")?;

    // Also eprintln so visible even if tracing console is buffered
    eprintln!("🚀 Binance Basis Monitor - Rust | {} symbols | log: {}", config.symbols.len(), config.log_file);

    info!("{}", "=".repeat(60));
    info!("🚀 Starting Binance Basis Monitor - Rust Implementation");
    info!("{}", "=".repeat(60));
    info!("Configuration loaded: {} symbols", config.symbols.len());
    info!("Log level: {}", config.log_level);
    info!("Log file: {}", config.log_file);

    Ok(())
}

/// Log basis data in a structured format.
pub fn log_basis_data(
    symbol: &str,
    spot_price: f64,
    futures_price: f64,
    basis: f64,
    ma_basis: Option<f64>,
    ema_basis: Option<f64>,
    z_score: Option<f64>,
) {
    let mut message = format!(
        "{} | 现货: {:.4}, 合约: {:.4} | 基差: {:.4}%",
        symbol.to_uppercase(),
        spot_price,
        futures_price,
        basis * 100.0
    );

    if let Some(ma) = ma_basis {
        message.push_str(&format!(" | MA: {:.4}%", ma * 100.0));
    }

    if let Some(ema) = ema_basis {
        message.push_str(&format!(" | EMA: {:.4}%", ema * 100.0));
    }

    if let Some(z) = z_score {
        message.push_str(&format!(" | Z-Score: {:.2}", z));
    }

    info!(basis_data = true, "{}", message);
}

/// Log performance metrics.
pub fn log_performance_metrics(stats: &QueueStats, symbol_count: usize) {
    info!(
        "Performance | Symbols: {} | Receive: {:.0} msg/s | Process: {:.0} msg/s | Backlog: {} | P99 Queue: {:.1}ms | P99 E2E: {:.1}ms",
        symbol_count,
        stats.receive_rate,
        stats.process_rate,
        stats.backlog,
        stats.latency_p99 * 1000.0,
        stats.latency_e2e_p99 * 1000.0
    );
}

/// Log comprehensive system status.
pub fn log_system_status(
    config: &Config,
    stats: &QueueStats,
    basis_calculator: Option<&crate::calculator::BasisCalculator>,
    indicator_calculator: Option<&crate::calculator::IndicatorCalculator>,
) {
    info!("{}", "=".repeat(60));
    info!("SYSTEM STATUS REPORT");
    info!("{}", "=".repeat(60));
    info!("Symbols: {} ({})", config.symbols.len(), if config.performance_test_mode { "TEST MODE" } else { "NORMAL" });
    info!("");
    info!("[QUEUE STATS]");
    info!("  Received: {}", stats.received_count);
    info!("  Processed: {}", stats.processed_count);
    info!("  Dropped: {}", stats.dropped_count);
    info!("  Queue Size: {}/{}", stats.queue_size, stats.max_size);
    info!("  Receive Rate: {:.0} msg/s", stats.receive_rate);
    info!("  Process Rate: {:.0} msg/s", stats.process_rate);
    info!("  Backlog: {}", stats.backlog);
    info!("");
    info!("[LATENCY]");
    info!("  P50: {:.1}ms", stats.latency_p50 * 1000.0);
    info!("  P90: {:.1}ms", stats.latency_p90 * 1000.0);
    info!("  P99: {:.1}ms", stats.latency_p99 * 1000.0);
    info!("  P999: {:.1}ms", stats.latency_p999 * 1000.0);
    info!("");
    info!("[E2E LATENCY] (Binance E → processed)");
    info!("  P50: {:.1}ms", stats.latency_e2e_p50 * 1000.0);
    info!("  P90: {:.1}ms", stats.latency_e2e_p90 * 1000.0);
    info!("  P99: {:.1}ms", stats.latency_e2e_p99 * 1000.0);
    info!("  P999: {:.1}ms", stats.latency_e2e_p999 * 1000.0);

    if let Some(calculator) = basis_calculator {
        info!("");
        info!("[BASIS CALCULATOR]");
        info!("  Window Interval: {}s", config.window_interval);
        // Note: We don't have a method to get symbol count from basis calculator
        // in the current implementation
    }

    if let Some(calculator) = indicator_calculator {
        info!("");
        info!("[INDICATOR CALCULATOR]");
        info!("  Window Size: {}", config.ema_window);
        info!("  Symbols with indicators: {}", calculator.symbol_count());
    }

    info!("");
    info!("{}", "=".repeat(60));
}

/// Log error with context.
pub fn log_error<E: std::error::Error + Send + Sync + 'static>(
    error: E,
    context: &str,
) {
    error!("{}: {}", context, error);
}

/// Log warning with context.
pub fn log_warning(context: &str) {
    warn!("{}", context);
}

/// Log informational message.
pub fn log_info(message: &str) {
    info!("{}", message);
}