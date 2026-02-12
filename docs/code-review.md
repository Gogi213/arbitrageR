# Code Review: Sprint 1-4 (Deep Dive)

**Date:** 2026-02-12
**Reviewer:** AI Assistant
**Scope:** All commits from ed07092 to d58c70f

## Executive Summary

| Severity | Count | Action Required |
|----------|-------|-----------------|
| CRITICAL | 6 | Fix before production |
| IMPORTANT | 5 | Fix in Sprint 5 |
| MINOR | 6 | Backlog |

**Key Findings:**
- Hardcoded symbol whitelist (anti-pattern) - symbols defined in 4 places
- Mutex/RwLock in hot path - causes latency spikes
- HashMap + Vec allocations in message processing

---

## CRITICAL Issues (HFT Violations)

### 1. Heap Allocation in Hot Path - Symbol Registration
**File:** `src/core/symbol.rs:87-120`
**Severity:** CRITICAL

```rust
fn register_dynamic(bytes: &[u8]) -> Option<Self> {
    let symbols = DYNAMIC_SYMBOLS.lock().ok()?;  // Mutex lock
    // ...
    symbols.insert(bytes.to_vec(), id);          // Vec allocation
    let leaked: &'static str = Box::leak(name.to_string().into_boxed_str()); // Heap leak
}
```

**Problem:** Dynamic symbol registration allocates heap memory and acquires Mutex lock. If called during message parsing (hot path), violates zero-allocation rule.

**Fix:** 
- Pre-register all symbols at startup (warm path)
- Return `Symbol::UNKNOWN` for unregistered symbols in hot path
- Or use lock-free concurrent data structure (DashMap)

---

### 2. HashMap in Hot Path - Bybit Ticker Cache
**File:** `src/exchanges/bybit/mod.rs:31`
**Severity:** CRITICAL

```rust
pub struct BybitWsClient {
    tickers: HashMap<Symbol, TickerData>,  // HashMap lookup = hashing
}
```

**Problem:** HashMap requires hashing on every lookup/insert. Hashing is O(N) where N = key length. For HFT, this violates the "no hashing in hot path" rule.

**Fix:**
- Use array indexed by Symbol ID: `[Option<TickerData>; MAX_SYMBOLS]`
- Or use `IndexMap` with pre-hashed keys

---

### 3. Vec Resize in Tracker Update
**File:** `src/hot_path/tracker.rs:111-119`
**Severity:** CRITICAL

```rust
fn ensure_symbol(&mut self, symbol: Symbol) {
    let id = symbol.as_raw() as usize;
    if id >= self.states.len() {
        self.states.resize(id + 1, SymbolState::new(...));  // Allocation!
    }
}
```

**Problem:** Vec resize allocates heap memory. Called on every ticker update.

**Fix:**
- Pre-allocate `Vec::with_capacity(Symbol::MAX_SYMBOLS as usize)` in `new()`
- Or use fixed-size array: `[Option<SymbolState>; MAX_SYMBOLS]`

---

### 4. Large Stack Allocation - MessageRouter
**File:** `src/hot_path/routing.rs:23-27`
**Severity:** CRITICAL

```rust
pub struct MessageRouter {
    ticker_handlers: [Option<TickerHandler>; MAX_ROUTES],  // 10,000 * 16 bytes = 160KB
    trade_handlers: [Option<TradeHandler>; MAX_ROUTES],    // Another 160KB
}
```

**Problem:** 320KB stack allocation. Can cause stack overflow, especially in embedded/tiered environments.

**Fix:**
- Use `Box<[Option<TickerHandler>; MAX_ROUTES]>` (heap allocation at construction)
- Or reduce `MAX_ROUTES` to realistic number (e.g., 1000)

---

### 5. Hardcoded Symbol Whitelist (Anti-Pattern)
**Files:** `src/core/symbol.rs`, `src/core/symbol_map.rs`, `src/main.rs`
**Severity:** CRITICAL

**Symbol lists defined in 3 places:**

1. `src/core/symbol.rs:49-60` - `from_bytes()` match statement:
```rust
match bytes {
    b"BTCUSDT" => return Some(Symbol::BTCUSDT),
    b"ETHUSDT" => return Some(Symbol::ETHUSDT),
    // ... 11 hardcoded symbols
}
```

2. `src/core/symbol.rs:154-164` - Constants:
```rust
pub const BTCUSDT: Self = Self(0);
pub const ETHUSDT: Self = Self(1);
// ... 11 constants
```

3. `src/main.rs:69-81` - Runtime list:
```rust
let symbols = vec![
    Symbol::BTCUSDT,
    Symbol::ETHUSDT,
    // ... 11 symbols
];
```

4. `src/core/symbol_map.rs:46-59` - Mapping table:
```rust
pub static SYMBOL_MAP: [SymbolInfo; 11] = [
    SymbolInfo::new(Symbol::BTCUSDT, "BTCUSDT"),
    // ... 11 entries
];
```

**Problems:**
- **DRY Violation:** Same 11 symbols in 4 places. Adding new symbol requires 4 code changes.
- **Not Scalable:** Can't track all liquid pairs. Only 11 hardcoded symbols.
- **Manual Maintenance:** Must manually add/remove symbols. No dynamic discovery.
- **Missed Opportunities:** High-volume pairs like 1000SHIBUSDT, WLDUSDT, BLURUSDT not tracked.

**Solution - Dynamic Symbol Loading:**
```rust
// At startup (warm path):
// 1. Fetch 24h volume from Binance REST API
// 2. Filter symbols with volume > $1,000,000
// 3. Register symbols dynamically
// 4. Pre-allocate SymbolState for each

async fn load_liquid_symbols() -> Result<Vec<Symbol>> {
    let response = reqwest::get("https://fapi.binance.com/fapi/v1/ticker/24hr").await?;
    let tickers: Vec<Ticker24h> = response.json().await?;
    
    tickers.into_iter()
        .filter(|t| t.quote_volume > 1_000_000.0) // > $1M 24h volume
        .filter_map(|t| Symbol::from_bytes(t.symbol.as_bytes()))
        .collect()
}
```

**Affected by this:**
- `src/core/symbol.rs:12-15` - Duplicate static declarations (also unused)
- `src/core/symbol.rs:87-120` - Dynamic registration has Mutex + allocation (CRITICAL #1)
- `src/engine.rs:124` - RwLock write on every update

---

### 6. Mutex/RwLock Contention in Hot Path
**Files:** `src/core/symbol.rs`, `src/engine.rs`
**Severity:** CRITICAL

**Lock operations found:**

1. `src/core/symbol.rs:97` - Mutex lock during symbol registration:
```rust
let symbols = DYNAMIC_SYMBOLS.lock().ok()?;  // BLOCKS!
```

2. `src/core/symbol.rs:104` - Mutable Mutex lock:
```rust
let mut symbols = DYNAMIC_SYMBOLS.lock().ok()?;  // BLOCKS!
```

3. `src/engine.rs:124` - RwLock write on every ticker:
```rust
let mut tracker = self.tracker.write().await;  // BLOCKS all readers!
```

**Problem:** Locks in hot path cause:
- Latency spikes (waiting for lock)
- Priority inversion
- Cascading delays under load

**Fix:**
- Use MPSC channel for ticker updates (already partially implemented in engine.rs)
- Pre-register all symbols at startup, no runtime registration
- Use lock-free data structures for shared state

---

## IMPORTANT Issues

### 7. Duplicate Static Declarations
**File:** `src/core/symbol.rs`
**Severity:** IMPORTANT

Static `DYNAMIC_SYMBOLS` and `DYNAMIC_NAMES` are declared at module level (lines 12-15) AND inside functions (lines 92-94, 128-129, 143-144).

**Problem:** Creates 4 separate static variables. Only the function-local ones are used.

**Fix:** Remove module-level declarations (lines 12-15) - they are dead code.

---

### 8. Unused Code (35 Compiler Warnings)
**Severity:** IMPORTANT

Unused imports and dead code:
- `src/exchanges/binance/mod.rs`: 6 unused imports, unused `parse_message`
- `src/exchanges/bybit/mod.rs`: 4 unused imports, unused `parse_message`, unused `request_id`
- `src/engine.rs`: unused `exchange`, `trade` variables
- `src/hot_path/routing.rs`: unused `HftError` import
- `src/infrastructure/api.rs`: 4 unused imports
- `src/ws/ping.rs`: unused `unhealthy_threshold` field
- `src/ws/pool.rs`: unused `id` field, multiple unused imports

**Fix:** Run `cargo fix --lib -p rust-hft` and `cargo fix --bin "rust-hft"`

---

### 9. Magic Numbers Without Constants
**File:** `src/hot_path/tracker.rs:57`, `src/engine.rs:127`
**Severity:** IMPORTANT

```rust
if event.spread.as_raw() > 50_000 {  // What is 50_000?
```

**Problem:** 50_000 represents 0.05% threshold but is not documented or configurable.

**Fix:**
```rust
const OPPORTUNITY_THRESHOLD_BPS: i64 = 50_000; // 0.05% in FixedPoint8 scale
```

---

### 10. Hardcoded Absolute Path
**File:** `src/infrastructure/api.rs:62`
**Severity:** IMPORTANT

```rust
let static_files = ServeDir::new("/root/arbitrageR/reference/frontend");
```

**Problem:** Won't work in other environments or deployment.

**Fix:** Use relative path from executable or config:
```rust
let static_path = std::env::current_dir()
    .unwrap()
    .join("reference/frontend");
```

---

### 11. O(N) Symbol Lookup
**File:** `src/core/symbol_map.rs:67-82`
**Severity:** IMPORTANT

```rust
pub fn get_name(symbol: Symbol, exchange: Exchange) -> Option<&'static str> {
    for info in SYMBOL_MAP.iter() {  // Linear scan
        if info.symbol == symbol {
            return Some(...);
        }
    }
}
```

**Problem:** O(N) lookup for every ticker message. For 11 symbols, this is acceptable, but not ideal.

**Fix:** Use array indexing by Symbol ID (O(1)):
```rust
static SYMBOL_MAP: [SymbolInfo; 11] = [...];
fn get_name(symbol: Symbol, ...) -> Option<&'static str> {
    SYMBOL_MAP.get(symbol.as_raw() as usize).map(...)
}
```

---

## MINOR Issues

### 12. Missing Test Coverage
**File:** `src/exchanges/parsing/bybit.rs:171-175`
**Severity:** MINOR

Tests are stubbed: `// ... existing tests would go here, simplified for brevity ...`

**Fix:** Add tests for:
- `parse_public_trade`
- `parse_ticker_update`
- `detect_message_type`

---

### 13. RingBuffer Iterator Logic
**File:** `src/infrastructure/ring_buffer.rs:40-42`
**Severity:** MINOR

```rust
pub fn iter(&self) -> impl Iterator<Item = &T> {
    self.buffer.iter().take(self.count)  // Wrong for wrapped buffer
}
```

**Problem:** When buffer wraps, this returns elements in wrong order (newest first).

**Fix:** Implement proper ring buffer iteration or document the behavior.

---

### 14. Debug Assert in Production Code
**File:** `src/hot_path/calculator.rs:39-41`
**Severity:** MINOR

```rust
debug_assert_eq!(binance.symbol, symbol);
```

**Problem:** `debug_assert` is stripped in release builds. For safety-critical code, consider regular `assert` or explicit check.

---

### 15. Unwrap in Hot Path
**File:** `src/hot_path/calculator.rs:50, 61`
**Severity:** MINOR

```rust
.and_then(|diff| diff.safe_div(binance.ask_price))
.unwrap_or(FixedPoint8::ZERO)
```

**Problem:** `unwrap_or` is correct, but the pattern is repeated. Consider helper method.

---

### 16. Timestamp Missing in bookTicker
**File:** `src/exchanges/parsing/binance.rs:111`
**Severity:** MINOR

```rust
let timestamp = 0;  // bookTicker doesn't have timestamp
```

**Problem:** Zero timestamp means stale data detection won't work.

**Fix:** Use `std::time::SystemTime::now()` or inject timestamp at call site.

---

### 17. Missing Documentation
**Severity:** MINOR

Several public APIs lack documentation:
- `SymbolMapper::from_exchange_name` edge cases
- `SpreadCalculator::calculate` return value for invalid inputs
- `RingBuffer::min_max` O(N) complexity

---

## Architecture Compliance

### Hot/Warm/Cold Separation: PARTIAL

| Component | Layer | Status |
|-----------|-------|--------|
| `FixedPoint8` | Hot | PASS |
| `Symbol` (static) | Hot | PASS |
| `Symbol` (dynamic) | Hot | **FAIL** - allocations |
| `TickerData/TradeData` | Hot | PASS |
| `BinanceParser` | Hot | PASS |
| `BybitParser` | Hot | PASS |
| `SpreadCalculator` | Hot | PASS |
| `MessageRouter` | Hot | **FAIL** - large stack |
| `ThresholdTracker` | Warm | **FAIL** - Vec resize |
| `BybitWsClient::tickers` | Warm | **FAIL** - HashMap |
| `RingBuffer` | Warm | PASS |
| `API Server` | Cold | PASS |
| `Engine` | Cold | PASS |

---

## Recommendations for Sprint 5

**Sprint Document:** `docs/sprints/sprint-05-optimization.md`

### Priority 1 (Before Production)
1. Implement dynamic symbol loading from exchange API (24h volume > $1M)
2. Pre-register all symbols at startup, remove hot-path registration
3. Replace `HashMap` with array in `BybitWsClient::tickers`
4. Pre-allocate `ThresholdTracker::states` at construction
5. Box the large arrays in `MessageRouter`
6. Eliminate Mutex/RwLock from hot path

### Priority 2 (Sprint 5)
1. Fix all 35 compiler warnings
2. Add configuration system (config.toml)
3. Fix hardcoded paths
4. Add missing tests for Bybit parser

### Priority 3 (Backlog)
1. Optimize `SymbolMapper` to O(1) lookup
2. Add proper ring buffer iteration
3. Add API documentation
4. Add benchmarks for hot path

---

## Files Reviewed

| File | Lines | Issues |
|------|-------|--------|
| `core/fixed_point.rs` | 500 | 0 |
| `core/symbol.rs` | 265 | 3 (CRITICAL x2, IMPORTANT) |
| `core/symbol_map.rs` | 166 | 1 (IMPORTANT) |
| `core/market_data.rs` | 316 | 0 |
| `hot_path/calculator.rs` | 151 | 1 (MINOR) |
| `hot_path/tracker.rs` | 177 | 2 (CRITICAL, IMPORTANT) |
| `hot_path/routing.rs` | 371 | 1 (CRITICAL) |
| `exchanges/binance/mod.rs` | 320 | 1 (IMPORTANT) |
| `exchanges/bybit/mod.rs` | 450 | 2 (CRITICAL, IMPORTANT) |
| `exchanges/parsing/binance.rs` | 283 | 1 (MINOR) |
| `exchanges/parsing/bybit.rs` | 176 | 1 (MINOR) |
| `infrastructure/api.rs` | 107 | 2 (IMPORTANT, MINOR) |
| `infrastructure/ring_buffer.rs` | 175 | 1 (MINOR) |
| `engine.rs` | 153 | 1 (CRITICAL) |

**Total:** ~3,500 lines reviewed

---

## Sprint 5 Plan

See `docs/sprints/sprint-05-optimization.md` for full implementation plan.

**Phases:**
1. Dynamic Symbol Discovery (load from API, filter by 24h volume)
2. Pre-Registration System (no runtime registration)
3. Array-Based Ticker Cache (replace HashMap)
4. Pre-Allocated Tracker (no Vec resize)
5. Boxed MessageRouter (no stack overflow)
6. Configuration System (config.toml)
7. Cleanup & Documentation
