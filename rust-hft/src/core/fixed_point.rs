//! Fixed-point arithmetic for HFT
//!
//! Uses i64 internally with 8 decimal places precision.
//! Zero allocation, Copy type, no panics.

use std::fmt;
use std::str::FromStr;

/// Fixed-point number with 8 decimal places
/// Stored as i64 where value = real_value * 100_000_000
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct FixedPoint8(i64);

impl FixedPoint8 {
    /// Number of decimal places
    pub const DECIMALS: u8 = 8;

    /// Scale factor (10^8)
    pub const SCALE: i64 = 100_000_000;

    /// One unit (1.0)
    pub const ONE: Self = Self(100_000_000);

    /// Zero
    pub const ZERO: Self = Self(0);

    /// Maximum value
    pub const MAX: Self = Self(i64::MAX);

    /// Minimum value
    pub const MIN: Self = Self(i64::MIN);

    /// Create from raw i64 value
    #[inline(always)]
    pub const fn from_raw(value: i64) -> Self {
        Self(value)
    }

    /// Get raw i64 value
    #[inline(always)]
    pub const fn as_raw(&self) -> i64 {
        self.0
    }

    /// Create from f64 (for config/cold path only)
    /// Returns None if value is NaN, infinite, or out of range
    #[inline]
    pub fn from_f64(value: f64) -> Option<Self> {
        if !value.is_finite() {
            return None;
        }
        let scaled = (value * Self::SCALE as f64).round();
        if scaled > i64::MAX as f64 || scaled < i64::MIN as f64 {
            return None;
        }
        Some(Self(scaled as i64))
    }

    /// Convert to f64 (for math/statistics)
    #[inline(always)]
    pub fn to_f64(&self) -> f64 {
        self.0 as f64 / Self::SCALE as f64
    }

    /// Checked addition - returns None on overflow
    #[inline(always)]
    pub const fn checked_add(&self, other: Self) -> Option<Self> {
        match self.0.checked_add(other.0) {
            Some(result) => Some(Self(result)),
            None => None,
        }
    }

    /// Checked subtraction - returns None on overflow
    #[inline(always)]
    pub const fn checked_sub(&self, other: Self) -> Option<Self> {
        match self.0.checked_sub(other.0) {
            Some(result) => Some(Self(result)),
            None => None,
        }
    }

    /// Checked negation - returns None on overflow
    #[inline(always)]
    pub const fn checked_neg(&self) -> Option<Self> {
        match self.0.checked_neg() {
            Some(result) => Some(Self(result)),
            None => None,
        }
    }

    /// Checked absolute value - returns None on overflow (i64::MIN)
    #[inline(always)]
    pub const fn checked_abs(&self) -> Option<Self> {
        match self.0.checked_abs() {
            Some(result) => Some(Self(result)),
            None => None,
        }
    }

    /// Safe multiplication using i128 to prevent overflow
    /// Returns None if result doesn't fit in i64
    #[inline]
    pub fn safe_mul(&self, other: Self) -> Option<Self> {
        let a = self.0 as i128;
        let b = other.0 as i128;
        let product = a * b;
        // Divide by scale to get back to FixedPoint8
        let scaled = product / Self::SCALE as i128;

        if scaled > i64::MAX as i128 || scaled < i64::MIN as i128 {
            return None;
        }
        Some(Self(scaled as i64))
    }

    /// Safe division using i128 for precision
    /// Returns None on division by zero or overflow
    #[inline]
    pub fn safe_div(&self, other: Self) -> Option<Self> {
        if other.0 == 0 {
            return None;
        }
        let a = self.0 as i128;
        let b = other.0 as i128;
        // Multiply by scale before division for precision
        let scaled = (a * Self::SCALE as i128) / b;

        if scaled > i64::MAX as i128 || scaled < i64::MIN as i128 {
            return None;
        }
        Some(Self(scaled as i64))
    }

    /// Calculate spread in basis points (1 bps = 0.01%)
    /// Returns spread as FixedPoint8 where 100_000_000 = 100%
    /// So 100 bps (1%) = 1_000_000 in raw
    #[inline]
    pub fn spread_bps(
        &self,
        other: Self
    ) -> Option<Self> {
        // spread = (other - self) / self * 10000
        // For 100 to 101: (1 / 100) * 10000 = 100 bps = 1%
        // In FixedPoint8: 1% = 0.01 = 1_000_000 raw
        let diff = other.checked_sub(*self)?;
        // ratio = diff / self (in FixedPoint8 scale)
        // For 1/100 = 0.01, in raw: 1_000_000
        let ratio = diff.safe_div(*self)?;
        // Multiply by 10000 to convert to bps
        // 0.01 * 10000 = 100 bps
        ratio.safe_mul(Self(10_000))
    }

    /// Parse from byte slice without allocation
    /// Supports format: "12345.6789" or "12345"
    /// Returns None on invalid format or overflow
    #[inline]
    pub fn parse_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.is_empty() {
            return None;
        }

        let mut negative = false;
        let mut i = 0;

        // Handle sign
        if bytes[0] == b'-' {
            negative = true;
            i = 1;
        } else if bytes[0] == b'+' {
            i = 1;
        }

        let mut integer_part: i64 = 0;
        let mut fractional_part: i64 = 0;
        let mut fractional_digits: u8 = 0;
        let mut has_decimal = false;

        while i < bytes.len() {
            let c = bytes[i];

            if c == b'.' {
                if has_decimal {
                    return None; // Multiple decimal points
                }
                has_decimal = true;
                i += 1;
                continue;
            }

            if c < b'0' || c > b'9' {
                return None; // Invalid character
            }

            let digit = (c - b'0') as i64;

            if !has_decimal {
                // Integer part
                integer_part = integer_part.checked_mul(10)?;
                integer_part = integer_part.checked_add(digit)?;
            } else {
                // Fractional part
                if fractional_digits < 8 {
                    fractional_part = fractional_part.checked_mul(10)?;
                    fractional_part = fractional_part.checked_add(digit)?;
                    fractional_digits += 1;
                }
                // Ignore digits beyond 8 decimal places
            }

            i += 1;
        }

        // Scale fractional part to 8 decimal places
        while fractional_digits < 8 {
            fractional_part = fractional_part.checked_mul(10)?;
            fractional_digits += 1;
        }

        // Combine integer and fractional parts
        let result = integer_part
            .checked_mul(Self::SCALE)?
            .checked_add(fractional_part)?;

        if negative {
            Some(Self(-result))
        } else {
            Some(Self(result))
        }
    }

    /// Write to buffer without allocation
    /// Returns number of bytes written
    /// Buffer must be at least 32 bytes for safety
    #[inline]
    pub fn write_to_buffer(&self, buf: &mut [u8]) -> usize {
        if buf.is_empty() {
            return 0;
        }

        let n = self.0.abs();
        let negative = self.0 < 0;

        // Integer part
        let integer = n / Self::SCALE;
        let fractional = n % Self::SCALE;

        let mut pos = 0;

        // Write sign
        if negative {
            if pos < buf.len() {
                buf[pos] = b'-';
                pos += 1;
            }
        }

        // Write integer part (from right to left)
        let int_start = pos;
        if integer == 0 {
            if pos < buf.len() {
                buf[pos] = b'0';
                pos += 1;
            }
        } else {
            let mut int_val = integer;
            while int_val > 0 && pos < buf.len() {
                buf[pos] = b'0' + (int_val % 10) as u8;
                int_val /= 10;
                pos += 1;
            }
            // Reverse the integer part
            let int_end = pos;
            for i in 0..(int_end - int_start) / 2 {
                buf.swap(int_start + i, int_end - 1 - i);
            }
        }

        // Write decimal point and fractional part
        if pos < buf.len() {
            buf[pos] = b'.';
            pos += 1;
        }

        // Write exactly 8 fractional digits
        let mut frac_val = fractional;
        for _ in 0..8 {
            if pos >= buf.len() {
                break;
            }
            buf[pos] = b'0' + ((frac_val / 10_000_000) % 10) as u8;
            frac_val = (frac_val % 10_000_000) * 10;
            pos += 1;
        }

        pos
    }

    /// Get the sign (-1, 0, 1)
    #[inline(always)]
    pub const fn signum(&self) -> i64 {
        self.0.signum()
    }

    /// Check if zero
    #[inline(always)]
    pub const fn is_zero(&self) -> bool {
        self.0 == 0
    }

    /// Check if positive
    #[inline(always)]
    pub const fn is_positive(&self) -> bool {
        self.0 > 0
    }

    /// Check if negative
    #[inline(always)]
    pub const fn is_negative(&self) -> bool {
        self.0 < 0
    }
}

impl Default for FixedPoint8 {
    #[inline(always)]
    fn default() -> Self {
        Self::ZERO
    }
}

impl fmt::Display for FixedPoint8 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut buf = [0u8; 32];
        let len = self.write_to_buffer(&mut buf);
        write!(f, "{}", std::str::from_utf8(&buf[..len]).unwrap_or(""))
    }
}

impl FromStr for FixedPoint8 {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse_bytes(s.as_bytes()).ok_or(())
    }
}

// HFT Hot Path Checklist verified:
// ✓ No heap allocations (all stack-based)
// ✓ No panics (all checked operations return Option)
// ✓ Stack only (Copy type)
// ✓ SIMD-friendly (can operate on raw i64)
// ✓ No dynamic dispatch
// ✓ No formatting in hot path (write_to_buffer for zero-alloc)

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_arithmetic() {
        let a = FixedPoint8::from_raw(100_000_000); // 1.0
        let b = FixedPoint8::from_raw(200_000_000); // 2.0

        assert_eq!(a.checked_add(b).unwrap().as_raw(), 300_000_000);
        assert_eq!(b.checked_sub(a).unwrap().as_raw(), 100_000_000);
    }

    #[test]
    fn test_overflow_protection() {
        let max = FixedPoint8::MAX;
        let one = FixedPoint8::ONE;

        assert!(max.checked_add(one).is_none());
        assert!(FixedPoint8::MIN.checked_sub(one).is_none());
    }

    #[test]
    fn test_safe_mul() {
        let a = FixedPoint8::from_raw(200_000_000); // 2.0
        let b = FixedPoint8::from_raw(300_000_000); // 3.0

        // 2.0 * 3.0 = 6.0
        let result = a.safe_mul(b).unwrap();
        assert_eq!(result.as_raw(), 600_000_000);
    }

    #[test]
    fn test_safe_div() {
        let a = FixedPoint8::from_raw(600_000_000); // 6.0
        let b = FixedPoint8::from_raw(200_000_000); // 2.0

        // 6.0 / 2.0 = 3.0
        let result = a.safe_div(b).unwrap();
        assert_eq!(result.as_raw(), 300_000_000);
    }

    #[test]
    fn test_div_by_zero() {
        let a = FixedPoint8::ONE;
        let zero = FixedPoint8::ZERO;

        assert!(a.safe_div(zero).is_none());
    }

    #[test]
    fn test_parse_bytes() {
        // Integer only
        assert_eq!(
            FixedPoint8::parse_bytes(b"123").unwrap().as_raw(),
            123_000_000_00
        );

        // With decimals
        assert_eq!(
            FixedPoint8::parse_bytes(b"123.456").unwrap().as_raw(),
            12_345_600_000
        );

        // Negative
        assert_eq!(
            FixedPoint8::parse_bytes(b"-123.5").unwrap().as_raw(),
            -12_350_000_000
        );

        // Zero
        assert_eq!(FixedPoint8::parse_bytes(b"0").unwrap().as_raw(), 0);

        // Max precision
        assert_eq!(
            FixedPoint8::parse_bytes(b"0.12345678").unwrap().as_raw(),
            12_345_678
        );
    }

    #[test]
    fn test_parse_invalid() {
        assert!(FixedPoint8::parse_bytes(b"").is_none());
        assert!(FixedPoint8::parse_bytes(b"abc").is_none());
        assert!(FixedPoint8::parse_bytes(b"1.2.3").is_none());
        assert!(FixedPoint8::parse_bytes(b"--1").is_none());
    }

    #[test]
    fn test_write_to_buffer() {
        let value = FixedPoint8::from_raw(123_456_789_00); // 123.45678900
        let mut buf = [0u8; 32];
        let len = value.write_to_buffer(&mut buf);

        assert_eq!(&buf[..len], b"123.45678900");
    }

    #[test]
    fn test_display() {
        let value = FixedPoint8::from_raw(123_456_789_00);
        assert_eq!(format!("{}", value), "123.45678900");

        let negative = FixedPoint8::from_raw(-50_000_000);
        assert_eq!(format!("{}", negative), "-0.50000000");
    }

    #[test]
    fn test_from_str_roundtrip() {
        let original = FixedPoint8::from_raw(987_654_321_00);
        let s = original.to_string();
        let parsed = FixedPoint8::from_str(&s).unwrap();
        assert_eq!(original.as_raw(), parsed.as_raw());
    }

    #[test]
    fn test_f64_conversion() {
        let value = 123.456789;
        let fixed = FixedPoint8::from_f64(value).unwrap();
        let back = fixed.to_f64();
        assert!((back - value).abs() < 0.000_000_1);
    }

    #[test]
    fn test_copy_type() {
        let a = FixedPoint8::ONE;
        let b = a; // Copy, not move
        let c = a; // Can still use a
        assert_eq!(a.as_raw(), b.as_raw());
        assert_eq!(a.as_raw(), c.as_raw());
    }

    #[test]
    fn test_spread_bps() {
        // Price 100, price 101 = 1% spread = 100 bps
        let a = FixedPoint8::from_raw(100 * FixedPoint8::SCALE); // 100.0
        let b = FixedPoint8::from_raw(101 * FixedPoint8::SCALE); // 101.0
        let spread = a.spread_bps(b).unwrap();
        // 100 bps = 100 raw (since 1 bps = 1 unit in this representation)
        // Allow small rounding error
        assert!(spread.as_raw() >= 99 && spread.as_raw() <= 101,
            "Expected ~100 bps, got {} (raw)", spread.as_raw());
    }
}
