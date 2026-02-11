# Sprint 5: Arbitrage Engine

**Goal**: Detect and execute arbitrage opportunities between Binance and Bybit.

## Phase 5.1: Spread Calculator
**Status**: PENDING  
**Objective**: Calculate price spreads between exchanges

### Tasks
- [ ] Design `SpreadCalculator`
- [ ] Maintain price cache for both exchanges
- [ ] Calculate bid-ask spreads
- [ ] Calculate cross-exchange spreads
- [ ] Update atomically (no torn reads)

### Interface
```rust
pub struct SpreadCalculator {
    // [symbol_id][exchange_idx] = ticker
    prices: [[Option<TickerData>; 2]; MAX_SYMBOLS],
}

impl SpreadCalculator {
    pub fn update_price(
        &mut self,
        exchange: Exchange,
        ticker: TickerData,
    );
    
    pub fn calculate_spread(
        &self,
        symbol: Symbol,
    ) -> Option<Spread> {
        // Binance bid vs Bybit ask
        // Bybit bid vs Binance ask
    }
}

#[derive(Copy, Clone)]
pub struct Spread {
    pub symbol: Symbol,
    pub binance_bid_bybit_ask: FixedPoint8, // Spread bps
    pub bybit_bid_binance_ask: FixedPoint8,
    pub timestamp: u64,
}
```

### HFT Checklist
- [ ] No allocation
- [ ] Lock-free updates (AtomicCell or channels)
- [ ] Cache-friendly layout
- [ ] No branches in hot path

### Tests
- [ ] Spread calculation accuracy
- [ ] Concurrent updates
- [ ] Stale data detection

---

## Phase 5.2: Opportunity Detector
**Status**: PENDING  
**Objective**: Identify profitable arbitrage opportunities

### Tasks
- [ ] Define `Opportunity` struct
- [ ] Set minimum spread threshold (e.g., 5 bps)
- [ ] Account for trading fees
- [ ] Account for slippage estimate
- [ ] Filter by volume requirements
- [ ] Rate limit opportunities (debounce)

### Interface
```rustn#[derive(Copy, Clone)]
pub struct Opportunity {
    pub symbol: Symbol,
    pub direction: ArbDirection, // Binance->Bybit or Bybit->Binance
    pub buy_exchange: Exchange,
    pub sell_exchange: Exchange,
    pub buy_price: FixedPoint8,
    pub sell_price: FixedPoint8,
    pub spread_bps: FixedPoint8, // After fees
    pub max_size: FixedPoint8,   // Limited by depth
    pub expected_profit: FixedPoint8,
    pub timestamp: u64,
}

pub struct OpportunityDetector {
    min_spread_bps: FixedPoint8,
    fees: Fees, // Both exchange fees
    min_profit_usd: FixedPoint8,
}

impl OpportunityDetector {
    pub fn detect(
        &self,
        spread: &Spread,
        depth: &OrderBookDepth,
    ) -> Option<Opportunity>;
}
```

### HFT Checklist
- [ ] No allocation in detect()
- [ ] Fast rejection (early exit)
- [ ] Copy type for Opportunity

### Tests
- [ ] Opportunity detection with fees
- [ ] Below threshold rejection
- [ ] Volume filtering

---

## Phase 5.3: Risk Manager
**Status**: PENDING  
**Objective**: Validate and limit arbitrage risks

### Tasks
- [ ] Maximum position size per symbol
- [ ] Maximum total exposure
- [ ] Maximum daily loss limit
- [ ] Correlation check (avoid similar positions)
- [ ] Market condition filters (high volatility)
- [ ] Cooldown between trades

### Interface
```rustnpub struct RiskManager {
    max_position_size: FixedPoint8,
    max_total_exposure: FixedPoint8,
    daily_loss_limit: FixedPoint8,
    cooldown_ms: u64,
    last_trade_time: [u64; MAX_SYMBOLS],
}

impl RiskManager {
    pub fn validate_opportunity(
        &self,
        opp: &Opportunity,
        positions: &PositionTracker,
        account: &AccountState,
    ) -> RiskDecision;
    
    pub fn record_trade(
        &mut self,
        symbol: Symbol,
        size: FixedPoint8,
    );
}

pub enum RiskDecision {
    Approve,
    RejectPositionLimit,
    RejectExposure,
    RejectCooldown,
    RejectMarketConditions,
}
```

### HFT Checklist
- [ ] O(1) risk checks
- [ ] No allocation
- [ ] Fast rejection path

### Tests
- [ ] Position limit enforcement
- [ ] Cooldown working
- [ ] Market condition filters

---

## Phase 5.4: Execution Engine
**Status**: PENDING  
**Objective**: Execute arbitrage trades atomically

### Tasks
- [ ] Design `ExecutionEngine`
- [ ] Place orders on both exchanges
- [ ] Handle partial fills
- [ ] Implement hedge logic
- [ ] Handle execution failures
- [ ] Position reconciliation after trade

### Interface
```rustnpub struct ExecutionEngine {
    binance: Arc<dyn OrderClient>,
    bybit: Arc<dyn OrderClient>,
    risk: RiskManager,
}

impl ExecutionEngine {
    pub async fn execute_arbitrage(
        &self,
        opp: Opportunity,
    ) -> Result<TradeResult> {
        // 1. Validate risk
        // 2. Place buy order
        // 3. Place sell order
        // 4. Wait for fills
        // 5. Handle partial fills
    }
    
    async fn place_hedge_order(
        &self,
        remaining: FixedPoint8,
        opp: &Opportunity,
    ) -> Result<()>;
}

pub struct TradeResult {
    pub buy_fill: FillData,
    pub sell_fill: FillData,
    pub actual_profit: FixedPoint8,
    pub execution_time_ms: u64,
}
```

### HFT Checklist
- [ ] Fast order placement
- [ ] Concurrent order submission
- [ ] Minimal allocations

### Tests
- [ ] Successful execution
- [ ] Partial fill handling
- [ ] Failed order recovery
- [ ] Hedge placement

---

## Phase 5.5: PnL Tracker
**Status**: PENDING  
**Objective**: Track realized and unrealized PnL

### Tasks
- [ ] Record all fills
- [ ] Calculate realized PnL per trade
- [ ] Track cumulative PnL
- [ ] Handle fees accurately
- [ ] Generate PnL reports (cold path)

### Interface
```rustnpub struct PnLTracker {
    fills: Vec<FillRecord>, // Cold path storage
    realized_pnl: FixedPoint8,
    total_fees: FixedPoint8,
    trade_count: u64,
}

impl PnLTracker {
    pub fn record_fill(&mut self,
        fill: &FillData,
        opp: &Opportunity,
    );
    
    pub fn get_stats(&self,
    ) -> PnLStats {
        PnLStats {
            realized_pnl: self.realized_pnl,
            total_fees: self.total_fees,
            net_pnl: self.realized_pnl - self.total_fees,
            trade_count: self.trade_count,
            avg_profit_per_trade: self.realized_pnl / self.trade_count as i64,
        }
    }
}
```

### Tests
- [ ] PnL calculation accuracy
- [ ] Fee tracking
- [ ] Report generation

---

## Sprint 5 Completion Criteria
- [ ] Opportunity detection <10μs
- [ ] Risk check <5μs
- [ ] Execution latency <100ms (both legs)
- [ ] Track PnL accurately
- [ ] Handle 10 opportunities/second
- [ ] Paper trading test complete
