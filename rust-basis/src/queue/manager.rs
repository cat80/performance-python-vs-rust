//! Message queue management for producer-consumer pattern.

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use crossbeam_channel::{Receiver, Sender, bounded, unbounded};
use atomic_counter::{AtomicCounter, RelaxedCounter};
use tracing::{debug, warn};

use crate::websocket::client::WebSocketMessage;

/// Manager for message queues and performance metrics.
pub struct QueueManager {
    /// Channel receiver for messages
    rx: Receiver<WebSocketMessage>,
    /// Channel sender for messages
    tx: Sender<WebSocketMessage>,
    /// Maximum queue size
    max_size: usize,
    /// Queue warning threshold
    warning_threshold: usize,

    // Performance counters
    /// Received message count
    received_count: Arc<RelaxedCounter>,
    /// Processed message count
    processed_count: Arc<RelaxedCounter>,
    /// Dropped message count
    dropped_count: Arc<RelaxedCounter>,

    // Latency tracking
    /// Latency samples (circular buffer)
    latencies: Vec<Duration>,
    /// Maximum number of latency samples to keep
    max_latency_samples: usize,
    /// Current index in circular buffer
    latency_index: AtomicUsize,

    // E2E latency (Binance E → mark_processed)
    /// E2E latency samples (circular buffer)
    e2e_latencies: Vec<Duration>,
    /// E2E latency index
    e2e_latency_index: AtomicUsize,

    /// Per-symbol counters
    symbol_stats: HashMap<String, SymbolStats>,

    /// Start time for rate calculation
    start_time: Instant,
}

/// Statistics for a symbol.
#[derive(Debug, Clone, Default)]
pub struct SymbolStats {
    /// Received count
    pub received: usize,
    /// Processed count
    pub processed: usize,
}

impl QueueManager {
    /// Create a new queue manager with bounded channel.
    pub fn new_bounded(max_size: usize, warning_threshold: usize) -> Self {
        let (tx, rx) = bounded(max_size);
        Self::new_with_channels(tx, rx, max_size, warning_threshold)
    }

    /// Create a new queue manager with unbounded channel.
    pub fn new_unbounded(warning_threshold: usize) -> Self {
        let (tx, rx) = unbounded();
        Self::new_with_channels(tx, rx, usize::MAX, warning_threshold)
    }

    /// Create a new queue manager with given channels.
    fn new_with_channels(
        tx: Sender<WebSocketMessage>,
        rx: Receiver<WebSocketMessage>,
        max_size: usize,
        warning_threshold: usize,
    ) -> Self {
        Self {
            tx,
            rx,
            max_size,
            warning_threshold,
            received_count: Arc::new(RelaxedCounter::new(0)),
            processed_count: Arc::new(RelaxedCounter::new(0)),
            dropped_count: Arc::new(RelaxedCounter::new(0)),
            latencies: vec![Duration::from_secs(0); 10000],
            max_latency_samples: 10000,
            latency_index: AtomicUsize::new(0),
            e2e_latencies: vec![Duration::from_secs(0); 10000],
            e2e_latency_index: AtomicUsize::new(0),
            symbol_stats: HashMap::new(),
            start_time: Instant::now(),
        }
    }

    /// Send a message to the queue.
    pub fn send(&mut self, message: WebSocketMessage) -> Result<(), WebSocketMessage> {
        let symbol = message.data["symbol"]
            .as_str()
            .unwrap_or("unknown")
            .to_string();
        // Add queue entry timestamp for latency
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
        let message_with_timestamp = WebSocketMessage {
            received_timestamp: now.as_secs(),
            queue_entry_millis: now.as_millis() as u64,
            ..message
        };
        match self.tx.try_send(message_with_timestamp) {
            Ok(()) => {
                // Update counters
                self.received_count.inc();

                // Update symbol stats
                // Note: This is not thread-safe for concurrent access to symbol_stats
                // In production, we'd use a concurrent hashmap or mutex
                // For this demo, we'll keep it simple
                let mut stats = self.symbol_stats.entry(symbol).or_default();
                stats.received += 1;

                // Check if queue is getting full
                if self.tx.len() > self.warning_threshold {
                    warn!(
                        "Queue approaching capacity: {}/{}",
                        self.tx.len(),
                        self.max_size
                    );
                }

                Ok(())
            }
            Err(crossbeam_channel::TrySendError::Full(msg)) => {
                // Queue full, drop message
                self.dropped_count.inc();
                warn!("Queue full, dropped message");
                Err(msg)
            }
            Err(crossbeam_channel::TrySendError::Disconnected(msg)) => {
                // Channel disconnected
                warn!("Channel disconnected");
                Err(msg)
            }
        }
    }

    /// Receive a message from the queue (blocking).
    pub fn recv(&self) -> Result<WebSocketMessage, crossbeam_channel::RecvError> {
        self.rx.recv()
    }

    /// Try to receive a message from the queue (non-blocking).
    pub fn try_recv(&self) -> Result<WebSocketMessage, crossbeam_channel::TryRecvError> {
        self.rx.try_recv()
    }

    /// Record that a message was received (for symbol_stats when producer increments received_count).
    pub fn record_received(&mut self, symbol: &str) {
        self.symbol_stats.entry(symbol.to_string()).or_default().received += 1;
    }

    /// Mark a message as processed.
    /// binance_event_millis: Binance E (event time ms) for E2E latency; None to skip.
    pub fn mark_processed(
        &mut self,
        symbol: &str,
        queue_latency: Option<Duration>,
        binance_event_millis: Option<u64>,
    ) {
        self.processed_count.inc();

        // Update symbol stats
        if let Some(stats) = self.symbol_stats.get_mut(symbol) {
            stats.processed += 1;
        } else {
            let mut stats = SymbolStats::default();
            stats.processed = 1;
            self.symbol_stats.insert(symbol.to_string(), stats);
        }

        // Record queue latency (actual or placeholder)
        let latency = queue_latency.unwrap_or_else(|| Duration::from_millis(1));
        let index = self.latency_index.fetch_add(1, Ordering::Relaxed) % self.max_latency_samples;
        self.latencies[index] = latency;

        // E2E latency: Binance E (event time) → now
        if let Some(binance_e) = binance_event_millis {
            let now_millis = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            let e2e_millis = now_millis.saturating_sub(binance_e);
            let e2e = Duration::from_millis(e2e_millis);
            let e2e_index = self.e2e_latency_index.fetch_add(1, Ordering::Relaxed) % self.max_latency_samples;
            self.e2e_latencies[e2e_index] = e2e;
        }
    }

    /// Get current queue statistics.
    pub fn get_stats(&self) -> QueueStats {
        let run_time = self.start_time.elapsed();
        let received = self.received_count.get();
        let processed = self.processed_count.get();
        let dropped = self.dropped_count.get();

        // Calculate rates
        let receive_rate = if run_time.as_secs() > 0 {
            received as f64 / run_time.as_secs_f64()
        } else {
            0.0
        };

        let process_rate = if run_time.as_secs() > 0 {
            processed as f64 / run_time.as_secs_f64()
        } else {
            0.0
        };

        // Calculate latency percentiles (cap at 2000 samples for speed)
        let mut sorted_latencies: Vec<Duration> = self.latencies
            .iter()
            .filter(|&&d| d > Duration::from_secs(0))
            .cloned()
            .collect();
        if sorted_latencies.len() > 2000 {
            sorted_latencies.truncate(2000);
        }
        sorted_latencies.sort_unstable();

        let n = sorted_latencies.len();
        let p50 = if n > 0 {
            sorted_latencies[(n * 50) / 100]
        } else {
            Duration::from_secs(0)
        };
        let p90 = if n > 1 {
            sorted_latencies[(n * 90) / 100]
        } else {
            Duration::from_secs(0)
        };
        let p99 = if n > 2 {
            sorted_latencies[(n * 99) / 100]
        } else {
            Duration::from_secs(0)
        };
        let p999 = if n > 3 {
            sorted_latencies[(n * 999) / 1000]
        } else {
            Duration::from_secs(0)
        };

        // E2E latency percentiles
        let mut sorted_e2e: Vec<Duration> = self.e2e_latencies
            .iter()
            .filter(|&&d| d > Duration::from_secs(0))
            .cloned()
            .collect();
        if sorted_e2e.len() > 2000 {
            sorted_e2e.truncate(2000);
        }
        sorted_e2e.sort_unstable();

        let ne = sorted_e2e.len();
        let e2e_p50 = if ne > 0 { sorted_e2e[(ne * 50) / 100] } else { Duration::from_secs(0) };
        let e2e_p90 = if ne > 1 { sorted_e2e[(ne * 90) / 100] } else { Duration::from_secs(0) };
        let e2e_p99 = if ne > 2 { sorted_e2e[(ne * 99) / 100] } else { Duration::from_secs(0) };
        let e2e_p999 = if ne > 3 { sorted_e2e[(ne * 999) / 1000] } else { Duration::from_secs(0) };

        QueueStats {
            queue_size: self.tx.len(),
            max_size: self.max_size,
            received_count: received,
            processed_count: processed,
            dropped_count: dropped,
            receive_rate,
            process_rate,
            backlog: received.saturating_sub(processed),
            run_time: run_time.as_secs_f64(),
            latency_p50: p50.as_secs_f64(),
            latency_p90: p90.as_secs_f64(),
            latency_p99: p99.as_secs_f64(),
            latency_p999: p999.as_secs_f64(),
            latency_e2e_p50: e2e_p50.as_secs_f64(),
            latency_e2e_p90: e2e_p90.as_secs_f64(),
            latency_e2e_p99: e2e_p99.as_secs_f64(),
            latency_e2e_p999: e2e_p999.as_secs_f64(),
            symbol_stats: self.symbol_stats.clone(),
        }
    }

    /// Reset performance statistics.
    pub fn reset_stats(&mut self) {
        self.received_count.reset();
        self.processed_count.reset();
        self.dropped_count.reset();
        self.latencies.fill(Duration::from_secs(0));
        self.latency_index.store(0, Ordering::Relaxed);
        self.e2e_latencies.fill(Duration::from_secs(0));
        self.e2e_latency_index.store(0, Ordering::Relaxed);
        self.symbol_stats.clear();
        self.start_time = Instant::now();
    }

    /// Get the channel sender for use by producers.
    pub fn sender(&self) -> Sender<WebSocketMessage> {
        self.tx.clone()
    }

    /// Get the received counter for producers to increment when sending (for receive_rate).
    pub fn received_counter(&self) -> Arc<RelaxedCounter> {
        Arc::clone(&self.received_count)
    }

    /// Get the channel receiver for use by consumers.
    pub fn receiver(&self) -> Receiver<WebSocketMessage> {
        self.rx.clone()
    }
}

/// Queue statistics.
#[derive(Debug, Clone)]
pub struct QueueStats {
    /// Current queue size
    pub queue_size: usize,
    /// Maximum queue size
    pub max_size: usize,
    /// Received message count
    pub received_count: usize,
    /// Processed message count
    pub processed_count: usize,
    /// Dropped message count
    pub dropped_count: usize,
    /// Receive rate (messages per second)
    pub receive_rate: f64,
    /// Process rate (messages per second)
    pub process_rate: f64,
    /// Queue backlog (received - processed)
    pub backlog: usize,
    /// Total run time in seconds
    pub run_time: f64,
    /// P50 latency in seconds
    pub latency_p50: f64,
    /// P90 latency in seconds
    pub latency_p90: f64,
    /// P99 latency in seconds
    pub latency_p99: f64,
    /// P999 latency in seconds
    pub latency_p999: f64,
    /// E2E P50 latency in seconds (Binance E → processed)
    pub latency_e2e_p50: f64,
    /// E2E P90 latency in seconds
    pub latency_e2e_p90: f64,
    /// E2E P99 latency in seconds
    pub latency_e2e_p99: f64,
    /// E2E P999 latency in seconds
    pub latency_e2e_p999: f64,
    /// Statistics by symbol
    pub symbol_stats: HashMap<String, SymbolStats>,
}

impl Default for QueueStats {
    fn default() -> Self {
        Self {
            queue_size: 0,
            max_size: 10000,
            received_count: 0,
            processed_count: 0,
            dropped_count: 0,
            receive_rate: 0.0,
            process_rate: 0.0,
            backlog: 0,
            run_time: 0.0,
            latency_p50: 0.0,
            latency_p90: 0.0,
            latency_p99: 0.0,
            latency_p999: 0.0,
            latency_e2e_p50: 0.0,
            latency_e2e_p90: 0.0,
            latency_e2e_p99: 0.0,
            latency_e2e_p999: 0.0,
            symbol_stats: HashMap::new(),
        }
    }
}