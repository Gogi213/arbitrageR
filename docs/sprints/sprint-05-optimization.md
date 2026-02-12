# Sprint 5: HFT Compliance & Dynamic Symbols

**Goal:** Fix all CRITICAL issues from code review and implement dynamic symbol loading.
**Focus:** Zero allocations in hot path, scalable symbol management.

## Background

Code review identified 6 CRITICAL issues:
1. Heap allocation in hot path (symbol registration)
2. HashMap in hot path (Bybit ticker cache)
3. Vec resize in tracker update
4. Large stack allocation (MessageRouter)
5. Hardcoded symbol whitelist (anti-pattern)
6. Mutex/RwLock contention in hot path

## Architecture Changes

```
Before:
  main.rs -> hardcoded vec![Symbol::BTCUSDT, ...] (11 symbols)
                    ↓
  Symbol::from_bytes() -> match on 11 hardcoded strings
                    ↓
  register_dynamic() -> Mutex + Vec allocation (HOT PATH VIOLATION)

After:
  main.rs -> load_liquid_symbols() (COLD PATH, startup)
                    ↓
  SymbolRegistry::register_all() (WARM PATH, one-time)
                    ↓
  Symbol::from_bytes() -> array lookup by pre-computed hash (HOT PATH, O(1))
```

## Phases

### Phase 5.1: Dynamic Symbol Discovery
**Status:** COMPLETE
**Objective:** Fetch liquid symbols from exchange APIs at startup.

- [x] Implement `SymbolDiscovery` struct with REST client
- [x] Fetch 24h ticker from Binance: `GET /fapi/v1/ticker/24hr`
- [x] Filter by `quoteVolume > 1,000,000 USDT`
- [x] Normalize symbol names (handle 1000PEPE, 1000SHIB, etc.)
- [x] Return `Vec<SymbolInfo>` sorted by volume

**API Response:**
```json
[
  {"symbol": "BTCUSDT", "quoteVolume": "15000000000"},
  {"symbol": "ETHUSDT", "quoteVolume": "8000000000"},
  ...
]
```

### Phase 5.2: Pre-Registration System
**Status:** COMPLETE
**Objective:** Register all symbols at startup, eliminate runtime registration.

- [x] Create `SymbolRegistry` struct
- [x] Store symbols in array indexed by ID: `[SymbolInfo; MAX_SYMBOLS]`
- [x] Add `SymbolRegistry::register_all(symbols: Vec<SymbolInfo>)` (warm path)
- [x] Remove `register_dynamic()` from hot path
- [x] Return `Symbol::UNKNOWN` for unregistered symbols in `from_bytes()`

**HFT Constraint:** Registration happens ONCE at startup. No locks in hot path.

### Phase 5.3: Array-Based Ticker Cache
**Status:** COMPLETE
**Objective:** Replace HashMap with array in BybitWsClient.

- [x] Change `BybitWsClient::tickers: HashMap<Symbol, TickerData>`
- [x] To `BybitWsClient::tickers: Box<[Option<TickerData>; MAX_SYMBOLS]>`
- [x] Update `merge_ticker()` to use array indexing
- [x] Benchmark: expect <10ns improvement per lookup

**Before:**
```rust
tickers: HashMap<Symbol, TickerData>  // O(N) hash + collision
```

**After:**
```rust
tickers: Box<[Option<TickerData>; MAX_SYMBOLS]>   // O(1) direct index
```

### Phase 5.4: Pre-Allocated Tracker
**Status:** COMPLETE
**Objective:** Eliminate Vec resize in ThresholdTracker.

- [x] Change `states: Vec<SymbolState>` to `states: Box<[Option<SymbolState>]>` (vec![] for non-Copy types)
- [x] Remove duplicate `impl ThresholdTracker` blocks
- [x] Direct array access by Symbol ID

**Before:**
```rust
fn ensure_symbol(&mut self, symbol: Symbol) {
    if id >= self.states.len() {
        self.states.resize(id + 1, ...);  // ALLOCATION!
    }
}
```

**After:**
```rust
fn get_or_create(&mut self, symbol: Symbol) -> &mut SymbolState {
    self.states[symbol.as_raw() as usize].get_or_insert_with(|| SymbolState::new(symbol))
}
```

### Phase 5.5: Boxed MessageRouter
**Status:** COMPLETE
**Objective:** Move large arrays to heap.

- [x] Change `[Option<TickerHandler>; MAX_ROUTES]` to `Box<[Option<TickerHandler>; MAX_ROUTES]>`
- [x] Same for `trade_handlers`
- [x] Update `new()` to use `Box::new()`
- [x] Verified: All tests pass, no stack overflow

**Stack Size Impact:** 2 arrays × 10,000 × 8 bytes = 160KB moved from stack to heap per router instance

### Phase 5.6: Configuration System
**Status:** PENDING
**Objective:** Make hardcoded values configurable.

- [ ] Create `Config` struct with:
  - `min_volume_24h: f64` (default: 1_000_000.0)
  - `opportunity_threshold_bps: i64` (default: 50_000)
  - `static_files_path: PathBuf`
  - `api_port: u16`
- [ ] Load from `config.toml` at startup
- [ ] Pass to all components

**config.toml:**
```toml
[hft]
min_volume_24h = 1_000_000.0
opportunity_threshold_bps = 50_000

[api]
port = 5000
static_path = "./reference/frontend"
```

### Phase 5.7: Cleanup & Documentation
**Status:** PENDING
**Objective:** Fix all compiler warnings and add docs.

- [ ] Run `cargo fix --lib -p rust-hft`
- [ ] Run `cargo fix --bin "rust-hft"`
- [ ] Remove duplicate static declarations in `symbol.rs`
- [ ] Add Bybit parser tests
- [ ] Document all public APIs

## HFT Hot Path Checklist

After Sprint 5, all hot path code must pass:

- [x] No Heap Allocations (Box allowed at construction)
- [ ] No HashMap or Hashing (replace with array)
- [ ] No Panics (all Option/Result)
- [ ] No Dynamic Dispatch (monomorphization only)
- [ ] No Formatting/Display in hot path
- [ ] No Clone of complex structures (Copy only)
- [ ] No Locks in hot path (pre-register everything)

## Metrics

| Metric | Before | After | Target |
|--------|--------|-------|--------|
| Symbols tracked | 11 | ~200 | Dynamic |
| Hot path allocations | 3+ | 0 | 0 |
| Stack size (MessageRouter) | 320KB | ~1KB | <10KB |
| Symbol lookup | O(N) hash | O(1) array | O(1) |
| Tracker update | Vec resize | Array index | O(1) |
| Locks in hot path | 2 | 0 | 0 |

## Dependencies

- `reqwest` for REST API (already in Cargo.toml)
- `toml` for config parsing (add to Cargo.toml)
- No new HFT-critical dependencies

## Risks

1. **Memory:** 5000 symbols × 64 bytes = 320KB per array. Acceptable.
2. **Startup time:** REST API call adds ~500ms. Acceptable (cold path).
3. **Symbol gaps:** IDs are dense (0..N) after pre-registration. OK.

## Next Steps

After Sprint 5:
- Sprint 6: Order Management System
- Sprint 7: Risk Management
- Sprint 8: Paper Trading
