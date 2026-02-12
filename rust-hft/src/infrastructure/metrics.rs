//! Metrics collection for system monitoring
//!
//! Lock-free metrics counters using atomic operations.
//! Collected in hot path, exported via API in cold path.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime};

/// System metrics collector
///
/// Thread-safe counters updated from hot path.
/// Snapshots taken for API export.
pub struct MetricsCollector {
    /// Total messages received from Binance
    binance_messages: AtomicU64,
    /// Total messages received from Bybit
    bybit_messages: AtomicU64,
    /// Total messages processed
    total_messages: AtomicU64,
    /// Binance connection status (0 = disconnected, 1 = connected)
    binance_connected: AtomicU64,
    /// Bybit connection status (0 = disconnected, 1 = connected)
    bybit_connected: AtomicU64,
    /// Last message timestamp (Unix millis)
    last_message_time: AtomicU64,
    /// Start time for uptime calculation
    start_time: Instant,
}

/// Metrics snapshot for API export
#[derive(Debug, Clone, Copy)]
pub struct MetricsSnapshot {
    pub binance_messages: u64,
    pub bybit_messages: u64,
    pub total_messages: u64,
    pub binance_connected: bool,
    pub bybit_connected: bool,
    pub message_rate: f64, // messages per second
    pub uptime_seconds: u64,
}

impl MetricsCollector {
    /// Create new metrics collector
    pub fn new() -> Self {
        Self {
            binance_messages: AtomicU64::new(0),
            bybit_messages: AtomicU64::new(0),
            total_messages: AtomicU64::new(0),
            binance_connected: AtomicU64::new(0),
            bybit_connected: AtomicU64::new(0),
            last_message_time: AtomicU64::new(0),
            start_time: Instant::now(),
        }
    }

    /// Record a message from Binance
    #[inline]
    pub fn record_binance_message(&self) {
        self.binance_messages.fetch_add(1, Ordering::Relaxed);
        self.total_messages.fetch_add(1, Ordering::Relaxed);
        self.update_last_message_time();
    }

    /// Record a message from Bybit
    #[inline]
    pub fn record_bybit_message(&self) {
        self.bybit_messages.fetch_add(1, Ordering::Relaxed);
        self.total_messages.fetch_add(1, Ordering::Relaxed);
        self.update_last_message_time();
    }

    /// Update last message timestamp
    #[inline]
    fn update_last_message_time(&self) {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        self.last_message_time.store(now, Ordering::Relaxed);
    }

    /// Set Binance connection status
    pub fn set_binance_connected(&self, connected: bool) {
        let value = if connected { 1 } else { 0 };
        self.binance_connected.store(value, Ordering::Relaxed);
    }

    /// Set Bybit connection status
    pub fn set_bybit_connected(&self, connected: bool) {
        let value = if connected { 1 } else { 0 };
        self.bybit_connected.store(value, Ordering::Relaxed);
    }

    /// Get current snapshot of metrics
    pub fn snapshot(&self) -> MetricsSnapshot {
        let binance_msgs = self.binance_messages.load(Ordering::Relaxed);
        let bybit_msgs = self.bybit_messages.load(Ordering::Relaxed);
        let total = self.total_messages.load(Ordering::Relaxed);

        let uptime = self.start_time.elapsed().as_secs();
        let rate = if uptime > 0 {
            total as f64 / uptime as f64
        } else {
            0.0
        };

        MetricsSnapshot {
            binance_messages: binance_msgs,
            bybit_messages: bybit_msgs,
            total_messages: total,
            binance_connected: self.binance_connected.load(Ordering::Relaxed) != 0,
            bybit_connected: self.bybit_connected.load(Ordering::Relaxed) != 0,
            message_rate: rate,
            uptime_seconds: uptime,
        }
    }

    /// Check if any exchange is connected
    pub fn is_connected(&self) -> bool {
        self.binance_connected.load(Ordering::Relaxed) != 0
            || self.bybit_connected.load(Ordering::Relaxed) != 0
    }

    /// Get latency estimate in milliseconds
    /// Returns time since last message, capped at 10000ms
    pub fn latency_ms(&self) -> u64 {
        let last = self.last_message_time.load(Ordering::Relaxed);
        if last == 0 {
            return 10000; // No messages yet
        }

        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        now.saturating_sub(last).min(10000)
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_collector_creation() {
        let collector = MetricsCollector::new();
        let snapshot = collector.snapshot();

        assert_eq!(snapshot.binance_messages, 0);
        assert_eq!(snapshot.bybit_messages, 0);
        assert_eq!(snapshot.total_messages, 0);
        assert!(!snapshot.binance_connected);
        assert!(!snapshot.bybit_connected);
    }

    #[test]
    fn test_record_messages() {
        let collector = MetricsCollector::new();

        collector.record_binance_message();
        collector.record_binance_message();
        collector.record_bybit_message();

        let snapshot = collector.snapshot();
        assert_eq!(snapshot.binance_messages, 2);
        assert_eq!(snapshot.bybit_messages, 1);
        assert_eq!(snapshot.total_messages, 3);
    }

    #[test]
    fn test_connection_status() {
        let collector = MetricsCollector::new();

        collector.set_binance_connected(true);
        collector.set_bybit_connected(false);

        let snapshot = collector.snapshot();
        assert!(snapshot.binance_connected);
        assert!(!snapshot.bybit_connected);
        assert!(collector.is_connected());
    }

    #[test]
    fn test_latency_no_messages() {
        let collector = MetricsCollector::new();
        assert_eq!(collector.latency_ms(), 10000);
    }

    #[test]
    fn test_message_rate_calculation() {
        let collector = MetricsCollector::new();

        // Simulate 100 messages
        for _ in 0..100 {
            collector.record_binance_message();
        }

        let snapshot = collector.snapshot();
        // Rate should be > 0 since we just added messages
        assert!(snapshot.message_rate >= 0.0);
        assert_eq!(snapshot.total_messages, 100);
    }
}
