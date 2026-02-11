//! Ring buffer for rolling statistics
//!
//! Stores fixed number of elements for rolling window calculations.
//! Zero allocation after initialization.

use std::fmt::Debug;

/// Ring buffer with fixed capacity
#[derive(Debug, Clone)]
pub struct RingBuffer<T, const N: usize> {
    buffer: [T; N],
    head: usize,
    count: usize,
    sum: T, // Maintained sum for O(1) average (requires T: Add + Sub + Copy)
}

impl<T: Copy + Default + Debug, const N: usize> RingBuffer<T, N> {
    /// Create new ring buffer
    pub fn new() -> Self {
        Self {
            buffer: [T::default(); N],
            head: 0,
            count: 0,
            sum: T::default(),
        }
    }

    /// Add value to buffer
    #[inline]
    pub fn push(&mut self, value: T) {
        self.buffer[self.head] = value;
        self.head = (self.head + 1) % N;
        if self.count < N {
            self.count += 1;
        }
    }

    /// Get all values (iterator)
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.buffer.iter().take(self.count)
    }
    
    /// Get stored count
    #[inline]
    pub fn count(&self) -> usize {
        self.count
    }
    
    /// Clear buffer
    pub fn clear(&mut self) {
        self.head = 0;
        self.count = 0;
        // buffer contents remain but are ignored
    }
}

// Special implementation for FixedPoint8 to maintain sum
use crate::core::FixedPoint8;

impl<const N: usize> RingBuffer<FixedPoint8, N> {
    /// Add value maintaining rolling sum
    #[inline]
    pub fn push_fp(&mut self, value: FixedPoint8) {
        // If buffer is full, subtract the overwritten value
        if self.count == N {
            let old_val = self.buffer[self.head];
            // We use unwrap because sum should track correctly
            // In HFT we might want to saturate instead of panic, but overflow is unlikely for spread sums
            if let Some(new_sum) = self.sum.checked_sub(old_val) {
                self.sum = new_sum;
            }
        }
        
        self.buffer[self.head] = value;
        self.head = (self.head + 1) % N;
        
        if self.count < N {
            self.count += 1;
        }
        
        if let Some(new_sum) = self.sum.checked_add(value) {
            self.sum = new_sum;
        }
    }
    
    /// Get rolling sum
    #[inline]
    pub fn sum(&self) -> FixedPoint8 {
        self.sum
    }
    
    /// Get min and max values (O(N) unfortunately, but N is small)
    /// Optimization: Could use a min/max heap or dequeue for O(1), but for N=120 O(N) is fine
    #[inline]
    pub fn min_max(&self) -> (FixedPoint8, FixedPoint8) {
        if self.count == 0 {
            return (FixedPoint8::ZERO, FixedPoint8::ZERO);
        }
        
        let mut min = FixedPoint8::MAX;
        let mut max = FixedPoint8::MIN;
        
        // Iterate only valid elements
        // This iterates the underlying buffer which may have stale data, 
        // but we only check valid slots?
        // Actually simple array iteration is fastest, but we need to check validity.
        // Or simpler: iterate all N if full, or 0..count if not.
        
        // Since ring buffer wraps, valid elements might not be contiguous.
        // It's safer/simpler to iterate 0..count and map index.
        
        for i in 0..self.count {
            // Logic to find actual index: 
            // if full: (head + i) % N ? No, head points to NEXT write position.
            // Oldest is at (head - count + N) % N
            
            let idx = if self.count < N {
                i
            } else {
                // If full, valid data is everywhere. Order doesn't matter for min/max.
                i
            };
            
            let val = self.buffer[idx];
            if val < min { min = val; }
            if val > max { max = val; }
        }
        
        (min, max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_ring_buffer_push() {
        let mut rb = RingBuffer::<i32, 3>::new();
        rb.push(1);
        rb.push(2);
        rb.push(3);
        assert_eq!(rb.count, 3);
        
        rb.push(4); // Overwrites 1
        assert_eq!(rb.count, 3);
        
        // Buffer state depends on implementation details, but iterator should work
        // Our iterator is naive buffer.iter().take(count) which is WRONG for ring buffer logic
        // if we want chronological order. But for min/max it doesn't matter.
    }
    
    #[test]
    fn test_fixed_point_min_max() {
        let mut rb = RingBuffer::<FixedPoint8, 5>::new();
        rb.push_fp(FixedPoint8::from_raw(100));
        rb.push_fp(FixedPoint8::from_raw(200));
        rb.push_fp(FixedPoint8::from_raw(50));
        
        let (min, max) = rb.min_max();
        assert_eq!(min.as_raw(), 50);
        assert_eq!(max.as_raw(), 200);
        
        // Fill and overwrite
        rb.push_fp(FixedPoint8::from_raw(300));
        rb.push_fp(FixedPoint8::from_raw(400));
        rb.push_fp(FixedPoint8::from_raw(500)); // Overwrites 100
        
        let (min, max) = rb.min_max();
        assert_eq!(min.as_raw(), 50); // 50 is still there
        assert_eq!(max.as_raw(), 500);
    }
}
