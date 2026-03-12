//! Binance Basis Monitor - Rust Implementation
//!
//! Main entry point for the application.

mod config;
mod websocket;
mod calculator;
mod queue;
mod ui;
mod metrics;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::sleep;
use anyhow::{Result, Context};
use tracing::{info, error, warn};

use crate::config::Config;
use crate::websocket::{MessageHandler, start_combined_websocket_client};
use crate::calculator::{BasisCalculator, IndicatorCalculator};
use crate::queue::QueueManager;
use crate::ui::{Dashboard, setup_logger, log_basis_data, log_performance_metrics, log_system_status};
use crate::metrics::MetricsCollector;

/// Main application structure.
struct BasisMonitor {
    /// Application configuration
    config: Config,
    /// Queue manager
    queue_manager: QueueManager,
    /// Message handler
    message_handler: MessageHandler,
    /// Basis calculator
    basis_calculator: BasisCalculator,
    /// Indicator calculator
    indicator_calculator: IndicatorCalculator,
    /// Metrics collector
    metrics_collector: MetricsCollector,
    /// Running flag
    running: bool,
}

impl BasisMonitor {
    /// Create a new basis monitor.
    fn new(config: Config) -> Result<Self> {
        // Setup logger
        setup_logger(&config).context("Failed to setup logger")?;

        // Create queue manager
        let queue_manager = QueueManager::new_bounded(
            config.queue_max_size,
            config.queue_warning_threshold,
        );

        let window_interval = config.window_interval;
        let ema_window = config.ema_window;
        Ok(Self {
            config,
            queue_manager,
            message_handler: MessageHandler::new(),
            basis_calculator: BasisCalculator::new(window_interval),
            indicator_calculator: IndicatorCalculator::new(ema_window),
            metrics_collector: MetricsCollector::new(60), // Collect every 60 seconds
            running: true,
        })
    }

    /// Start WebSocket clients (combined stream, max 120 symbols per connection).
    async fn start_websocket_clients(&self) -> Result<()> {
        let tx = self.queue_manager.sender();
        let received_counter = Some(self.queue_manager.received_counter());

        let spot_urls = self.config.get_spot_combined_ws_urls();
        let futures_urls = self.config.get_futures_combined_ws_urls();

        info!(
            "Starting combined WebSocket: {} spot connections, {} futures connections for {} symbols",
            spot_urls.len(),
            futures_urls.len(),
            self.config.symbols.len()
        );

        let conn_count = spot_urls.len() + futures_urls.len();
        for url in &spot_urls {
            let url = url.clone();
            let tx_clone = tx.clone();
            let rc_clone = received_counter.clone();
            tokio::spawn(async move {
                if let Err(e) = start_combined_websocket_client(url, "spot".to_string(), tx_clone, rc_clone).await {
                    error!("Spot combined WebSocket error: {}", e);
                }
            });
        }

        for url in &futures_urls {
            let url = url.clone();
            let tx_clone = tx.clone();
            let rc_clone = received_counter.clone();
            tokio::spawn(async move {
                if let Err(e) = start_combined_websocket_client(url, "futures".to_string(), tx_clone, rc_clone).await {
                    error!("Futures combined WebSocket error: {}", e);
                }
            });
        }

        info!("Started {} combined WebSocket connections", conn_count);
        Ok(())
    }

    /// Process messages from the queue.
    async fn process_messages(&mut self) -> Result<()> {
        info!("Starting message processor");

        let mut last_log_time = std::time::Instant::now();

        while self.running {
            match self.queue_manager.try_recv() {
                Ok(message) => {
                    let symbol_for_stats = message.data["symbol"]
                        .as_str()
                        .unwrap_or("unknown")
                        .to_string();
                    let binance_e = message.data.get("E").and_then(|v| v.as_u64());
                    // Process message through handler
                    if let Some((spot_data, futures_data)) = self.message_handler.process_message(message) {
                        let symbol = spot_data.symbol.clone();

                        // Calculate basis
                        if let Ok(Some(basis_data)) = self.basis_calculator.calculate_basis(
                            &spot_data.data,
                            &futures_data.data,
                            &symbol,
                        ) {
                            // Add to indicator calculator
                            self.indicator_calculator.add_data(
                                &basis_data.symbol,
                                basis_data.basis,
                                basis_data.spot_price,
                            );

                            // Calculate indicators
                            if let Ok(Some(indicators)) = self.indicator_calculator.calculate_indicators(&basis_data.symbol) {
                                // Log basis data every 10 seconds
                                if last_log_time.elapsed() >= Duration::from_secs(10) {
                                    log_basis_data(
                                        &basis_data.symbol,
                                        basis_data.spot_price,
                                        basis_data.futures_price,
                                        basis_data.basis,
                                        Some(indicators.ma_basis),
                                        Some(indicators.ema_basis),
                                        Some(indicators.z_score),
                                    );
                                    last_log_time = std::time::Instant::now();
                                }
                            }
                        }
                    }

                    // Mark as processed
                    self.queue_manager.mark_processed(&symbol_for_stats, None, binance_e);

                    // Cleanup stale prices periodically
                    if self.queue_manager.get_stats().processed_count % 1000 == 0 {
                        self.message_handler.cleanup_stale_prices(10); // 10 seconds max age
                    }
                }
                Err(crossbeam_channel::TryRecvError::Empty) => {
                    // No messages, sleep a bit
                    sleep(Duration::from_millis(10)).await;
                }
                Err(crossbeam_channel::TryRecvError::Disconnected) => {
                    error!("Message channel disconnected");
                    break;
                }
            }
        }

        info!("Message processor stopped");
        Ok(())
    }

    /// Run periodic logging tasks.
    async fn run_logger(&mut self) -> Result<()> {
        info!("Starting logger task");

        while self.running {
            sleep(Duration::from_secs(self.config.log_output_interval)).await;

            // Get current stats
            let stats = self.queue_manager.get_stats();

            // Log performance metrics
            log_performance_metrics(&stats, self.config.symbols.len());

            // Log detailed system status every minute
            if stats.run_time as u64 % 60 < self.config.log_output_interval as u64 {
                log_system_status(
                    &self.config,
                    &stats,
                    Some(&self.basis_calculator),
                    Some(&self.indicator_calculator),
                );
            }

            // Collect and log system metrics if needed
            if self.metrics_collector.should_collect() {
                if let Ok(metrics) = self.metrics_collector.collect(&stats) {
                    info!("System metrics: {}", crate::metrics::format_metrics_for_log(&metrics));
                }
            }
        }

        info!("Logger task stopped");
        Ok(())
    }

    /// Run the dashboard UI (requires exclusive QueueManager - not used in current run).
    #[allow(dead_code)]
    async fn run_dashboard(&mut self) -> Result<()> {
        info!("Starting dashboard UI");
        // Dashboard requires QueueManager ownership - use Dashboard::new_with_stats for shared stats
        anyhow::bail!("Dashboard integration pending - QueueManager does not implement Clone");
    }

    /// Run the main application.
    async fn run(mut self) -> Result<()> {
        info!("Starting Binance Basis Monitor");

        self.running = true;

        // Start WebSocket clients
        self.start_websocket_clients().await?;

        // Share monitor and stats across tasks
        let config = self.config.clone();
        let monitor = Arc::new(Mutex::new(self));
        let shared_stats = Arc::new(StdMutex::new(crate::queue::manager::QueueStats::default()));
        let shutdown = Arc::new(AtomicBool::new(false));

        // Start message processor (batch process, minimal stats update to avoid blocking)
        let processor_handle = tokio::spawn({
            let monitor = Arc::clone(&monitor);
            let shared_stats = Arc::clone(&shared_stats);
            let shutdown = Arc::clone(&shutdown);
            async move {
                let mut last_log = std::time::Instant::now();
                let mut last_stats_update = std::time::Instant::now();
                let mut processed_count = 0u64;
                loop {
                    if shutdown.load(Ordering::SeqCst) {
                        break;
                    }
                    let (should_exit, had_any) = {
                        let mut guard = monitor.lock().await;
                        if !guard.running || shutdown.load(Ordering::SeqCst) {
                            break;
                        }
                        let mut had_any = false;
                        // Batch process up to 16 messages per lock (smaller = lower P99 latency)
                        for _ in 0..16 {
                            match guard.queue_manager.try_recv() {
                            Ok(message) => {
                                had_any = true;
                                let symbol_for_stats = message.data["symbol"]
                                    .as_str()
                                    .unwrap_or("unknown")
                                    .to_string();
                                let binance_e = message.data.get("E").and_then(|v| v.as_u64());
                                let queue_entry_millis = message.queue_entry_millis;
                                let queue_latency = if queue_entry_millis > 0 {
                                    let now_millis = std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .unwrap_or_default()
                                        .as_millis() as u64;
                                    Some(std::time::Duration::from_millis(
                                        now_millis.saturating_sub(queue_entry_millis),
                                    ))
                                } else {
                                    None
                                };
                                guard.queue_manager.record_received(&symbol_for_stats);
                                if let Some((spot_data, futures_data)) = guard.message_handler.process_message(message) {
                                    let symbol = spot_data.symbol.clone();
                                    if let Ok(Some(basis_data)) = guard.basis_calculator.calculate_basis(
                                        &spot_data.data,
                                        &futures_data.data,
                                        &symbol,
                                    ) {
                                        guard.indicator_calculator.add_data(
                                            &basis_data.symbol,
                                            basis_data.basis,
                                            basis_data.spot_price,
                                        );
                                        if last_log.elapsed() >= Duration::from_secs(10) {
                                            if let Ok(Some(indicators)) = guard.indicator_calculator.calculate_indicators(&basis_data.symbol) {
                                                log_basis_data(
                                                    &basis_data.symbol,
                                                    basis_data.spot_price,
                                                    basis_data.futures_price,
                                                    basis_data.basis,
                                                    Some(indicators.ma_basis),
                                                    Some(indicators.ema_basis),
                                                    Some(indicators.z_score),
                                                );
                                                last_log = std::time::Instant::now();
                                            }
                                        }
                                    }
                                }
                                guard.queue_manager.mark_processed(&symbol_for_stats, queue_latency, binance_e);
                                processed_count += 1;
                                // Update shared stats only every 100 ms or 100 messages (get_stats is expensive)
                                if processed_count % 100 == 0 || last_stats_update.elapsed() >= Duration::from_millis(100) {
                                    if let Ok(mut s) = shared_stats.lock() {
                                        *s = guard.queue_manager.get_stats();
                                    }
                                    last_stats_update = std::time::Instant::now();
                                }
                            }
                            Err(crossbeam_channel::TryRecvError::Empty) => break,
                            Err(crossbeam_channel::TryRecvError::Disconnected) => {
                                error!("Message channel disconnected");
                                drop(guard);
                                return Ok(());
                            }
                        }
                        }
                        (false, had_any)
                    };
                    if should_exit {
                        break;
                    }
                    // When queue empty: yield instead of sleep - like Python's await queue.get()
                    // sleep(1ms) added 1ms latency; yield_now() wakes in microseconds when msg arrives
                    if !had_any {
                        tokio::task::yield_now().await;
                    }
                }
                Ok::<(), anyhow::Error>(())
            }
        });

        // Start logger task
        let logger_handle = tokio::spawn({
            let monitor = Arc::clone(&monitor);
            let shared_stats = Arc::clone(&shared_stats);
            let shutdown = Arc::clone(&shutdown);
            async move {
                loop {
                    sleep(Duration::from_secs(10)).await;
                    if shutdown.load(Ordering::SeqCst) {
                        break;
                    }
                    let (running, stats, symbol_count) = {
                        let guard = monitor.lock().await;
                        let stats = guard.queue_manager.get_stats();
                        if let Ok(mut s) = shared_stats.lock() {
                            *s = stats.clone();
                        }
                        (guard.running, stats, guard.config.symbols.len())
                    };
                    if !running || shutdown.load(Ordering::SeqCst) {
                        break;
                    }
                    log_performance_metrics(&stats, symbol_count);
                }
                Ok::<(), anyhow::Error>(())
            }
        });

        // Run dashboard in blocking thread (TUI)
        let dashboard_handle = tokio::task::spawn_blocking({
            let shared_stats = Arc::clone(&shared_stats);
            let shutdown = Arc::clone(&shutdown);
            move || {
                if let Ok(mut dashboard) = Dashboard::new_with_shared_stats(config, shared_stats, shutdown) {
                    let _ = dashboard.run();
                }
            }
        });

        // Wait for dashboard to exit (user pressed 'q'), then signal shutdown
        let _ = dashboard_handle.await;
        shutdown.store(true, Ordering::SeqCst);

        // Wait for processor and logger to finish
        let _ = tokio::try_join!(processor_handle, logger_handle);

        info!("Application stopped");
        Ok(())
    }

    /// Stop the application.
    fn stop(&mut self) {
        self.running = false;
        info!("Stopping application...");
    }
}

/// Main entry point.
#[tokio::main]
async fn main() -> Result<()> {
    // Immediate feedback - use eprintln so visible even when TUI takes over stdout
    eprintln!("Starting Binance Basis Monitor (Rust)...");
    eprintln!("Logs: logs/rust-basis.log (see LOG_FILE in .env)");
    eprintln!("Press Q to exit dashboard.\n");
    use std::io::Write;
    let _ = std::io::stderr().flush();

    // Load configuration
    let config = Config::load().context("Failed to load configuration")?;

    // Create application
    let app = BasisMonitor::new(config).context("Failed to create application")?;

    // Run application
    if let Err(e) = app.run().await {
        error!("Application error: {}", e);
        Err(e)
    } else {
        Ok(())
    }
}
