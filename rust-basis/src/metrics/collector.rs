//! Performance metrics collection.

use std::time::{Duration, Instant};
use sysinfo::{System, SystemExt, ProcessExt, Pid};
use anyhow::Result;

/// Performance metrics collected from the system.
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    /// Timestamp when metrics were collected
    pub timestamp: Instant,
    /// CPU usage percentage
    pub cpu_percent: f32,
    /// Memory usage in MB
    pub memory_mb: f32,
    /// Number of threads
    pub thread_count: usize,
    /// Receive message rate (msg/s)
    pub receive_rate: f64,
    /// Process message rate (msg/s)
    pub process_rate: f64,
    /// Queue backlog
    pub queue_backlog: usize,
    /// P99 latency in seconds
    pub latency_p99: f64,
}

/// Collector for performance metrics.
pub struct MetricsCollector {
    /// System information
    system: System,
    /// Process ID (if available)
    pid: Option<Pid>,
    /// Last collection time
    last_collection: Instant,
    /// Collection interval
    collection_interval: Duration,
}

impl MetricsCollector {
    /// Create a new metrics collector.
    pub fn new(collection_interval_secs: u64) -> Self {
        let mut system = System::new_all();
        system.refresh_all();

        // Try to find our own process
        let pid = sysinfo::get_current_pid().ok().and_then(|pid| {
            system.refresh_process(pid);
            Some(pid)
        });

        Self {
            system,
            pid,
            last_collection: Instant::now(),
            collection_interval: Duration::from_secs(collection_interval_secs),
        }
    }

    /// Collect performance metrics.
    pub fn collect(&mut self, queue_stats: &crate::queue::manager::QueueStats) -> Result<PerformanceMetrics> {
        // Only collect if enough time has passed
        if self.last_collection.elapsed() < self.collection_interval {
            return Err(anyhow::anyhow!("Collection interval not reached"));
        }

        // Refresh system information
        self.system.refresh_all();

        // Get process information if available
        let (cpu_percent, memory_mb, thread_count) = if let Some(pid) = self.pid {
            if let Some(process) = self.system.process(pid) {
                (
                    process.cpu_usage(),
                    process.memory() as f32 / 1024.0 / 1024.0, // Convert to MB
                    0, // Thread count not available in sysinfo 0.29
                )
            } else {
                (0.0, 0.0, 0)
            }
        } else {
            (0.0, 0.0, 0)
        };

        // Get queue statistics
        let receive_rate = queue_stats.receive_rate;
        let process_rate = queue_stats.process_rate;
        let queue_backlog = queue_stats.backlog;
        let latency_p99 = queue_stats.latency_p99;

        let metrics = PerformanceMetrics {
            timestamp: Instant::now(),
            cpu_percent,
            memory_mb,
            thread_count,
            receive_rate,
            process_rate,
            queue_backlog,
            latency_p99,
        };

        self.last_collection = Instant::now();

        Ok(metrics)
    }

    /// Check if it's time to collect metrics.
    pub fn should_collect(&self) -> bool {
        self.last_collection.elapsed() >= self.collection_interval
    }

    /// Get the collection interval.
    pub fn collection_interval(&self) -> Duration {
        self.collection_interval
    }

    /// Set the collection interval.
    pub fn set_collection_interval(&mut self, interval_secs: u64) {
        self.collection_interval = Duration::from_secs(interval_secs);
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new(60) // Default to 60 seconds
    }
}

/// Format metrics for display.
pub fn format_metrics(metrics: &PerformanceMetrics) -> String {
    format!(
        "CPU: {:.1}% | Memory: {:.1} MB | Threads: {} | Receive: {:.0}/s | Process: {:.0}/s | Backlog: {} | P99: {:.1}ms",
        metrics.cpu_percent,
        metrics.memory_mb,
        metrics.thread_count,
        metrics.receive_rate,
        metrics.process_rate,
        metrics.queue_backlog,
        metrics.latency_p99 * 1000.0
    )
}

/// Format metrics for logging.
pub fn format_metrics_for_log(metrics: &PerformanceMetrics) -> String {
    format!(
        "CPU={:.1}% Memory={:.1}MB Threads={} Receive={:.0}/s Process={:.0}/s Backlog={} P99={:.1}ms",
        metrics.cpu_percent,
        metrics.memory_mb,
        metrics.thread_count,
        metrics.receive_rate,
        metrics.process_rate,
        metrics.queue_backlog,
        metrics.latency_p99 * 1000.0
    )
}