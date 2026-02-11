# Sprint 6: Infrastructure & Observability

**Goal**: Monitoring, metrics, and operational tooling (all cold path).

## Phase 6.1: Metrics Collection
**Status**: PENDING  
**Objective**: Track system performance without impacting hot path

### Tasks
- [ ] Design lock-free metrics counters
- [ ] Track messages per second
- [ ] Track order latency percentiles
- [ ] Track spread distribution
- [ ] Use atomic counters (no locks)
- [ ] Periodic export to metrics backend

### Interface
```rust
pub struct Metrics {
    messages_received: AtomicU64,
    messages_sent: AtomicU64,
    orders_placed: AtomicU64,
    orders_filled: AtomicU64,
    latency_histogram: [AtomicU64; 10], // Buckets
}

impl Metrics {
    pub fn increment_messages_received();
    pub fn record_latency_us(latency: u64);
    pub fn snapshot() -> MetricsSnapshot; // For export
}
```

### Cold Path Only
- Metrics updated from hot path (atomic increment only)
- Aggregation and export in separate task
- No allocation in hot path

### Tests
- [ ] Concurrent metric updates
- [ ] Snapshot consistency
- [ ] Export functionality

---

## Phase 6.2: Structured Logging
**Status**: PENDING  
**Objective**: JSON logging for all cold path events

### Tasks
- [ ] Configure `tracing` with JSON subscriber
- [ ] Log all orders (placement, fill, cancel)
- [ ] Log opportunities and trades
- [ ] Log errors and warnings
- [ ] Structured fields (not just strings)
- [ ] Async writer to avoid blocking

### Interface
```rust
pub fn log_order_placement(
    symbol: Symbol,
    side: OrderSide,
    qty: FixedPoint8,
    order_id: &str,
);

pub fn log_trade_executed(
    opp: &Opportunity,
    result: &TradeResult,
);

pub fn log_error(
    error: &Error,
    context: &str,
);
```

### Cold Path Only
- No logging in hot path (message processing)
- Batch log writes
- Separate I/O thread

### Tests
- [ ] JSON format validation
- [ ] Log rotation
- [ ] Performance impact (should be minimal)

---

## Phase 6.3: Health Monitoring
**Status**: PENDING  
**Objective**: Monitor system health and alert on issues

### Tasks
- [ ] Connection health checks
- [ ] Latency monitoring (p50, p99)
- [ ] Error rate tracking
- [ ] Disk space monitoring (logs)
- [ ] Alert on critical issues

### Interface
```rust
pub struct HealthMonitor {
    connection_states: [ConnectionState; MAX_CONNECTIONS],
    alert_thresholds: AlertThresholds,
}

impl HealthMonitor {
    pub fn check_health(&self,
    ) -> Vec<HealthAlert>;
    
    pub fn is_healthy(&self,
    ) -> bool;
}

pub enum HealthAlert {
    ConnectionDown(Exchange),
    HighLatency(Exchange, u64), // p99 latency
    HighErrorRate(f64),         // errors per second
    LowDiskSpace(u64),          // MB remaining
}
```

### Tests
- [ ] Health check logic
- [ ] Alert generation
- [ ] Recovery detection

---

## Phase 6.4: Configuration Management
**Status**: PENDING  
**Objective**: Hot-reloadable configuration

### Tasks
- [ ] Load config from JSON/TOML
- [ ] Support environment variables
- [ ] Watch file for changes
- [ ] Hot reload without restart
- [ ] Validate config changes

### Interface
```rust
pub struct Config {
    pub exchanges: ExchangeConfig,
    pub risk: RiskConfig,
    pub strategy: StrategyConfig,
}

pub struct ExchangeConfig {
    pub binance: BinanceConfig,
    pub bybit: BybitConfig,
}

pub struct RiskConfig {
    pub max_position_size: f64,
    pub max_exposure: f64,
    pub min_spread_bps: f64,
}
```

### Tests
- [ ] Config loading
- [ ] Hot reload
- [ ] Invalid config rejection

---

## Phase 6.5: Graceful Shutdown
**Status**: PENDING  
**Objective**: Clean shutdown with position reconciliation

### Tasks
- [ ] Handle SIGTERM/SIGINT
- [ ] Cancel pending orders
- [ ] Close positions (configurable)
- [ ] Flush logs and metrics
- [ ] Save state to disk

### Interface
```rust
pub async fn graceful_shutdown(
    engine: &mut ArbitrageEngine,
    timeout: Duration,
) -> Result<()> {
    // 1. Stop accepting new opportunities
    // 2. Cancel pending orders
    // 3. Optionally close positions
    // 4. Flush data
    // 5. Disconnect
}
```

### Tests
- [ ] Shutdown with pending orders
- [ ] Timeout handling
- [ ] State saving

---

## Sprint 6 Completion Criteria
- [ ] Metrics export working
- [ ] Logs structured and queryable
- [ ] Health checks operational
- [ ] Config hot-reload works
- [ ] Graceful shutdown tested
- [ ] No impact on hot path latency
