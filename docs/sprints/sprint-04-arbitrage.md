# Sprint 4: Arbitrage Engine & Screener

**Goal**: Implement the core arbitrage logic and expose data to the Dashboard frontend.
**Focus**: Revival of `dashboard.html` with live data from Rust backend.

## Architecture
```
[Exchanges] -> [MessageRouter] -> [SpreadCalculator] -> [ThresholdTracker] -> [API Server] -> [Dashboard UI]
     ^                                   (Hot Path)          (Warm Path)       (Cold Path)
     |
(WebSockets)
```

## Phases

### Phase 4.1: Spread Calculator (Hot Path)
**Status**: COMPLETE ✅
**Objective**: Calculate spreads between exchanges with zero allocation.
- [x] Implement `SpreadCalculator` struct
- [x] Formula: `Spread = (Bid_A - Ask_B) / Ask_B` (and vice versa)
- [x] Output: `SpreadEvent` (Symbol, Spread, Side, Timestamp)
- [x] Benchmark target: <50ns per calculation (Actual: ~38ns)

### Phase 4.2: Threshold Tracker (Warm Path)
**Status**: COMPLETE ✅
**Objective**: Aggregate spread statistics for the Screener.
- [x] Implement `ThresholdTracker` to store symbol state
- [x] Metrics:
    - `CurrentSpread`: Latest calculated spread
    - `SpreadRange`: `Max - Min` over rolling window (2 mins)
    - `Hits`: Count of opportunities > threshold
- [x] Ring Buffer implementation for rolling stats
- [x] Benchmark: ~62ns per update (Target <100ns)

### Phase 4.3: API Server (Cold Path)
**Objective**: Serve data to `dashboard.html`.
- [ ] Integrate `axum` web server
- [ ] Implement `GET /api/screener/stats`
- [ ] JSON Serialization compatible with `store.js`
- [ ] Serve static files (`dashboard.html`, css, js)

### Phase 4.4: Integration & Validation
**Objective**: Connect all components.
- [ ] Wiring: WS Clients -> Calculator -> Tracker -> API
- [ ] Run full application
- [ ] Verify Dashboard updates in browser
- [ ] Performance validation (latency from tick to API update)

## Data Structures (Reference)

**Screener API Response (`store.js` expectation):**
```json
[
  {
    "symbol": "BTCUSDT",
    "currentSpread": 0.0015,
    "spreadRange": 0.0005,
    "hits": 12,
    "estHalfLife": 4.5,
    "isSpreadNA": false
  }
]
```

## HFT Constraints
- **Calculator**: Hot path rules apply (no alloc, no panic).
- **Tracker**: Warm path (minimal alloc, mostly updates in place).
- **API**: Cold path (allocations allowed for JSON serialization).
