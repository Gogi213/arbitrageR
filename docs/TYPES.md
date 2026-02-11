# HFT Type System Guide

This document describes the numeric types used in the HFT arbitrage bot, adapted from C# best practices to Rust.

## Type Overview

| Type | Rust Equivalent | Use Case | Performance |
|------|-----------------|----------|-------------|
| **FixedPoint8** | `i64` | Hot path, storage | 🚀 Fastest |
| **double** | `f64` | Math, statistics | 🧮 Fast (10-20x faster than decimal) |
| **decimal** | `rust_decimal::Decimal` | Public API, config | 🧊 Slow (128-bit) |
| **Int128** | `i128` | Math safety | 🛡️ Safe intermediate |

---

## 1. FixedPoint8 (i64) — Hot Path & Storage 🚀

**Primary type for the core system.**

### Concept
64-bit signed integer storing price multiplied by 100,000,000 (1e8).

### Usage
- `TickerData`, `TradeData` (in-memory storage)
- `BestBidTracker` (price comparison)
- WebSocket parsing (zero-allocation)
- Order book calculations
- Position tracking

### Rust Implementation
```rust
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct FixedPoint8(i64);

impl FixedPoint8 {
    pub const SCALE: i64 = 100_000_000; // 1e8
    pub const ONE: Self = Self(100_000_000);
    
    // Atomic operations for hot path
    pub fn atomic_load(&self) -> i64 {
        use std::sync::atomic::{AtomicI64, Ordering};
        // Cast to AtomicI64 for atomic ops
    }
}
```

### Advantages
- **Fastest**: Single CPU instruction for arithmetic
- **Atomic**: Can use `AtomicI64` for lock-free updates
- **Stack-only**: No heap allocation, `Copy` type
- **Precise**: No floating-point errors

### Example
```rust
// Price 25000.50 stored as
let price = FixedPoint8::from_decimal(25000.50);
// Internal: 2500050000000i64

// Arithmetic with overflow protection
let result = price.checked_mul(quantity)?;
```

---

## 2. f64 — Warm Path & Math 🧮

**Used for heavy mathematical computations and statistics.**

### Concept
Double-precision floating-point number.

### Usage
- Deviation analysis (calculating percentages)
- OHLCV calculations (aggregations)
- Statistical indicators (RSI, Bollinger bands if needed)
- Spread calculations in basis points

### Rust Implementation
```rust
// SIMD-friendly math
use std::simd::f64x4;

pub fn calculate_spread_bps(bid: FixedPoint8, ask: FixedPoint8) -> f64 {
    let bid_f = bid.to_f64();
    let ask_f = ask.to_f64();
    ((ask_f - bid_f) / bid_f) * 10_000.0 // Convert to basis points
}
```

### Advantages
- **Fast**: 10-20x faster than decimal
- **SIMD**: Hardware-level vectorization support
- **Familiar**: Standard IEEE 754

### Disadvantages
- **Micro-errors**: Possible 0.000000001 precision loss
- **NOT for money**: Never use for storing monetary values
- **Non-deterministic**: NaN, infinity edge cases

### Safety
```rust
impl FixedPoint8 {
    pub fn to_f64(self) -> f64 {
        self.0 as f64 / Self::SCALE as f64
    }
    
    pub fn from_f64(val: f64) -> Option<Self> {
        // Check for NaN, infinity
        if !val.is_finite() {
            return None;
        }
        let scaled = (val * Self::SCALE as f64).round() as i64;
        Some(Self(scaled))
    }
}
```

---

## 3. Decimal — Cold Path & Public API 🧊

**Legacy and boundaries of the system.**

### Concept
128-bit decimal with fixed precision. Slow but absolutely accurate.

### Usage
- Public API responses (JSON serialization)
- Configuration files (limits, volumes in USD)
- Logging (for readability)
- Frontend-facing data

### Rust Implementation
```rust
use rust_decimal::Decimal;
use rust_decimal::prelude::*;

// Only for API/config
pub struct PublicTradeData {
    pub symbol: Symbol,
    pub price: Decimal,  // Human-readable
    pub quantity: Decimal,
}

impl FixedPoint8 {
    pub fn to_decimal(self) -> Decimal {
        Decimal::from(self.0) / Decimal::from(100_000_000i64)
    }
}
```

### Advantages
- **100% accuracy**: Financial precision guaranteed
- **Human-readable**: Exact decimal representation

### Disadvantages
- **Slow**: 128-bit operations, allocation-heavy
- **Large**: 16 bytes (vs 8 for i64)
- **Not Copy**: Can require heap allocation in some cases

### Rule
**NEVER use Decimal in hot path. Convert to FixedPoint8 immediately.**

---

## 4. i128 — Math Safety 🛡

**Specific type for intermediate calculations.**

### Usage
- `RatioService` (EMA calculations)
- Multiplication of two FixedPoint8 values
- Any operation that might overflow i64

### Rust Implementation
```rust
impl FixedPoint8 {
    /// Multiply two FixedPoint8 values safely
    pub fn safe_mul(self, other: Self) -> Option<Self> {
        // Cast to i128 to prevent overflow
        let a = self.0 as i128;
        let b = other.0 as i128;
        
        // Multiply in 128-bit space
        let product = a * b;
        
        // Divide by scale (1e8) to get back to FixedPoint8
        let scaled = product / Self::SCALE as i128;
        
        // Check if result fits in i64
        if scaled > i64::MAX as i128 || scaled < i64::MIN as i128 {
            return None;
        }
        
        Some(Self(scaled as i64))
    }
    
    /// Division with precision
    pub fn safe_div(self, other: Self) -> Option<Self> {
        let a = self.0 as i128;
        let b = other.0 as i128;
        
        // Multiply by scale before division for precision
        let scaled = (a * Self::SCALE as i128) / b;
        
        if scaled > i64::MAX as i128 || scaled < i64::MIN as i128 {
            return None;
        }
        
        Some(Self(scaled as i64))
    }
}
```

### Why i128?
When multiplying two i64 values:
- Max i64: ~9e18
- Max i64 * Max i64: ~8e37
- i128 max: ~1.7e38

**Safe for intermediate calculations!**

---

## Type Conversion Map

### Hot Path → Warm Path
```rust
let fixed_price = FixedPoint8::from_raw(2500050000000i64);
let float_price = fixed_price.to_f64(); // For math
```

### Hot Path → Cold Path
```rust
let fixed_price = FixedPoint8::from_raw(2500050000000i64);
let decimal_price = fixed_price.to_decimal(); // For API
```

### String Parsing (Zero-Copy)
```rustnimpl FixedPoint8 {
    /// Parse from byte slice without allocation
    pub fn parse_bytes(bytes: &[u8]) -> Option<Self> {
        // Custom parser, no String allocation
        // "25000.50" → 2500050000000i64
    }
}
```

---

## Memory Layout

### FixedPoint8 (i64)
```
Size: 8 bytes
Alignment: 8 bytes
Copy: Yes
Stack: Yes
```

### f64
```
Size: 8 bytes
Alignment: 8 bytes
Copy: Yes
Stack: Yes
```

### Decimal (rust_decimal)
```
Size: 16 bytes
Alignment: 8 bytes
Copy: Yes (but slower)
Stack: Yes (but heavy)
```

### i128
```
Size: 16 bytes
Alignment: 16 bytes
Copy: Yes
Stack: Yes
Use: Only for intermediate math
```

---

## Performance Guidelines

### ✅ DO
- Use `FixedPoint8` for all hot path calculations
- Use `i128` for intermediate math
- Convert to `f64` only for statistics
- Convert to `Decimal` only at API boundaries

### ❌ DON'T
- Use `Decimal` in message processing
- Use `f64` for price storage
- Box `FixedPoint8` (unnecessary)
- Use `i128` for storage (wastes memory)

---

## Atomic Operations

For lock-free hot path updates:

```rust
use std::sync::atomic::{AtomicI64, Ordering};

pub struct AtomicPrice {
    value: AtomicI64,
}

impl AtomicPrice {
    pub fn load(&self) -> FixedPoint8 {
        FixedPoint8(self.value.load(Ordering::Relaxed))
    }
    
    pub fn store(&self, price: FixedPoint8) {
        self.value.store(price.0, Ordering::Relaxed);
    }
}
```

---

## Summary

1. **FixedPoint8 (i64)**: Core type for everything hot
2. **f64**: Math and statistics only
3. **Decimal**: API boundaries only
4. **i128**: Math safety intermediate

**Golden Rule**: Start with FixedPoint8, expand to larger types only when necessary, convert back immediately.
