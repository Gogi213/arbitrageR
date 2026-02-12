//! Time-window buffer for rolling statistics over time
//!
//! Stores values with timestamps and evicts entries older than window duration.
//! Used for calculating min/max over a time window (e.g., 2 minutes).

use crate::core::FixedPoint8;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Entry with timestamp
#[derive(Debug, Clone, Copy)]
struct TimedEntry {
    value: FixedPoint8,
    timestamp: Instant,
}

/// Time-window buffer for maintaining values within a time window
///
/// Efficiently tracks min/max by using a deque-based approach.
/// Evicts entries older than the window duration on each push.
#[derive(Debug, Clone)]
pub struct TimeWindowBuffer {
    /// Window duration (e.g., 2 minutes)
    window: Duration,
    /// Entries ordered by timestamp (oldest first)
    entries: VecDeque<TimedEntry>,
    /// Current minimum value
    min: FixedPoint8,
    /// Current maximum value
    max: FixedPoint8,
    /// Whether min/max need recalculation
    dirty: bool,
}

impl TimeWindowBuffer {
    /// Create new time-window buffer with specified duration
    pub fn new(window: Duration) -> Self {
        Self {
            window,
            entries: VecDeque::with_capacity(1024),
            min: FixedPoint8::ZERO,
            max: FixedPoint8::ZERO,
            dirty: false,
        }
    }

    /// Push a new value with current timestamp
    /// Evicts old entries outside the window
    pub fn push(&mut self, value: FixedPoint8) {
        let now = Instant::now();

        // Add new entry
        self.entries.push_back(TimedEntry {
            value,
            timestamp: now,
        });

        // Evict old entries
        self.evict_old(now);

        // Update min/max if needed
        if self.entries.len() == 1 {
            // First entry
            self.min = value;
            self.max = value;
            self.dirty = false;
        } else if value < self.min {
            self.min = value;
        } else if value > self.max {
            self.max = value;
        }
    }

    /// Evict entries older than window duration
    fn evict_old(&mut self, now: Instant) {
        let cutoff = now - self.window;

        // Remove entries older than cutoff
        while let Some(front) = self.entries.front() {
            if front.timestamp < cutoff {
                // Check if we're evicting min or max
                if front.value == self.min || front.value == self.max {
                    self.dirty = true;
                }
                self.entries.pop_front();
            } else {
                break;
            }
        }
    }

    /// Recalculate min/max from all entries
    fn recalc_min_max(&mut self) {
        if self.entries.is_empty() {
            self.min = FixedPoint8::ZERO;
            self.max = FixedPoint8::ZERO;
            self.dirty = false;
            return;
        }

        let mut min = FixedPoint8::MAX;
        let mut max = FixedPoint8::MIN;

        for entry in &self.entries {
            if entry.value < min {
                min = entry.value;
            }
            if entry.value > max {
                max = entry.value;
            }
        }

        self.min = min;
        self.max = max;
        self.dirty = false;
    }

    /// Get min and max values
    /// Returns (min, max)
    pub fn min_max(&mut self) -> (FixedPoint8, FixedPoint8) {
        // Evict old entries before returning
        self.evict_old(Instant::now());

        if self.dirty {
            self.recalc_min_max();
        }

        if self.entries.is_empty() {
            return (FixedPoint8::ZERO, FixedPoint8::ZERO);
        }

        (self.min, self.max)
    }

    /// Get current entry count
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if buffer is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear all entries
    pub fn clear(&mut self) {
        self.entries.clear();
        self.min = FixedPoint8::ZERO;
        self.max = FixedPoint8::ZERO;
        self.dirty = false;
    }
}

impl Default for TimeWindowBuffer {
    fn default() -> Self {
        Self::new(Duration::from_secs(120)) // 2 minutes default
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_time_window_basic() {
        let mut buf = TimeWindowBuffer::new(Duration::from_secs(60));

        buf.push(FixedPoint8::from_raw(100));
        buf.push(FixedPoint8::from_raw(200));
        buf.push(FixedPoint8::from_raw(50));

        let (min, max) = buf.min_max();
        assert_eq!(min.as_raw(), 50);
        assert_eq!(max.as_raw(), 200);
    }

    #[test]
    fn test_time_window_eviction() {
        let mut buf = TimeWindowBuffer::new(Duration::from_millis(100));

        buf.push(FixedPoint8::from_raw(100));
        buf.push(FixedPoint8::from_raw(200));

        // Wait for entries to expire
        thread::sleep(Duration::from_millis(150));

        // Add new entry, should evict old ones
        buf.push(FixedPoint8::from_raw(300));

        let (min, max) = buf.min_max();
        assert_eq!(min.as_raw(), 300);
        assert_eq!(max.as_raw(), 300);
    }

    #[test]
    fn test_empty_buffer() {
        let mut buf = TimeWindowBuffer::new(Duration::from_secs(60));
        let (min, max) = buf.min_max();
        assert_eq!(min.as_raw(), 0);
        assert_eq!(max.as_raw(), 0);
    }

    #[test]
    fn test_range_calculation() {
        // Test the range2m calculation: |min| + max
        let mut buf = TimeWindowBuffer::new(Duration::from_secs(120));

        // Negative min, positive max (arbitrage opportunity)
        buf.push(FixedPoint8::from_raw(-50_000)); // -0.05%
        buf.push(FixedPoint8::from_raw(100_000)); // +0.10%

        let (min, max) = buf.min_max();
        // range2m = |min| + max = 0.05% + 0.10% = 0.15%
        let range = min
            .checked_abs()
            .and_then(|abs_min| abs_min.checked_add(max))
            .unwrap_or(FixedPoint8::ZERO);
        assert_eq!(range.as_raw(), 150_000);
    }

    #[test]
    fn test_same_sign_na() {
        // Test is_spread_na: when min and max have same sign
        let mut buf = TimeWindowBuffer::new(Duration::from_secs(120));

        // All positive (no arbitrage)
        buf.push(FixedPoint8::from_raw(50_000));
        buf.push(FixedPoint8::from_raw(100_000));

        let (min, max) = buf.min_max();
        let is_spread_na =
            (min.is_positive() && max.is_positive()) || (min.is_negative() && max.is_negative());
        assert!(is_spread_na);
    }
}
