# Sprint 4: REST API & Order Management

**Goal**: Order placement, cancellation, and account management with minimal latency.

## Phase 4.1: REST Client Foundation
**Status**: PENDING  
**Objective**: HTTP client optimized for trading APIs

### Tasks
- [ ] Wrap `reqwest` or use `hyper` directly
- [ ] Configure connection pooling
- [ ] Set TCP optimizations (NODELAY, buffers)
- [ ] Implement request signing
- [ ] Handle rate limits (cold path tracking)
- [ ] Retry logic with backoff

### Interface
```rust
pub struct RestClient {
    http: Client,
    base_url: Url,
    rate_limiter: RateLimiter, // cold path
}

impl RestClient {
    pub fn new(base_url: &str) -> Self;
    pub async fn get(&self, path: &str) -> Result<Response>;
    pub async fn post(&self, path: &str, body: Vec<u8>) -> Result<Response>;
    pub async fn delete(&self, path: &str) -> Result<Response>;
}
```

### HFT Checklist
- [ ] Connection reuse (keep-alive)
- [ ] Pre-serialized request bodies
- [ ] Fast path: minimal allocations
- [ ] Timeout handling

### Tests
- [ ] GET request
- [ ] POST with body
- [ ] Connection reuse
- [ ] Timeout behavior

---

## Phase 4.2: Request Signing
**Status**: PENDING  
**Objective**: HMAC-SHA256 signing for authenticated requests

### Tasks
- [ ] Implement HMAC-SHA256 (use `hmac` + `sha2` crates)
- [ ] Binance signing format
- [ ] Bybit signing format (V5)
- [ ] Timestamp generation
- [ ] Signature hex encoding
- [ ] Pre-allocated buffers

### Interface
```rust
pub struct RequestSigner {
    secret: [u8; 64],  // Fixed size for HMAC key
    buffer: Vec<u8>,    // Reusable buffer
}

impl RequestSigner {
    pub fn new(secret: &str) -> Self;
    
    // Binance: sign(query_string + timestamp)
    pub fn sign_binance(&mut self, 
        query: &str, 
        timestamp: u64
    ) -> &str;
    
    // Bybit: sign(timestamp + api_key + recv_window + payload)
    pub fn sign_bybit(
        &mut self,
        timestamp: u64,
        api_key: &str,
        recv_window: u32,
        payload: &str,
    ) -> &str;
}
```

### HFT Checklist
- [ ] No allocation in sign()
- [ ] Reusable buffers
- [ ] Zero-copy where possible
- [ ] Constant-time comparison (if needed)

### Tests
- [ ] Binance signature verification
- [ ] Bybit signature verification
- [ ] Benchmark: <1μs per sign

---

## Phase 4.3: Order Types & Structures
**Status**: PENDING  
**Objective**: Order representation for both exchanges

### Tasks
- [ ] Define `Order` struct
- [ ] Define `OrderSide`, `OrderType`, `TimeInForce` enums
- [ ] Support market, limit, conditional orders
- [ ] Position side (Buy/Sell + Long/Short)
- [ ] Order ID generation

### Interface
```rust
#[derive(Copy, Clone)]
pub struct Order {
    pub symbol: Symbol,
    pub side: OrderSide,       // Buy/Sell
    pub order_type: OrderType, // Market/Limit/Conditional
    pub qty: FixedPoint8,
    pub price: Option<FixedPoint8>, // None for market
    pub time_in_force: TimeInForce,
    pub client_order_id: [u8; 36], // UUID as bytes
    pub reduce_only: bool,
    pub close_on_trigger: bool, // Bybit specific
}

#[derive(Copy, Clone)]
pub enum OrderSide {
    Buy = 1,
    Sell = 2,
}

#[derive(Copy, Clone)]
pub enum OrderType {
    Market = 1,
    Limit = 2,
    Conditional = 3,
}
```

### HFT Checklist
- [ ] Copy type (stack only)
- [ ] No allocation
- [ ] Efficient serialization

### Tests
- [ ] Order creation
- [ ] Serialization roundtrip
- [ ] Invalid order rejection

---

## Phase 4.4: Order Placement
**Status**: PENDING  
**Objective**: Place orders on both exchanges

### Tasks
- [ ] Implement `place_order` for Binance
- [ ] Implement `place_order` for Bybit
- [ ] Serialize order to JSON
- [ ] Handle response parsing
- [ ] Return order ID
- [ ] Error handling

### Interface
```rust
pub trait OrderClient: Send + Sync {
    async fn place_order(&self, 
        order: Order
    ) -> Result<OrderResponse>;
    
    async fn cancel_order(
        &self,
        symbol: Symbol,
        order_id: &str,
    ) -> Result<()>;
    
    async fn get_order(
        &self,
        symbol: Symbol,
        order_id: &str,
    ) -> Result<OrderStatus>;
}

pub struct OrderResponse {
    pub order_id: String,  // Exchange assigned ID
    pub client_order_id: [u8; 36],
    pub status: OrderStatus,
}
```

### HFT Checklist
- [ ] Fast serialization
- [ ] Minimal allocations
- [ ] Quick response parsing

### Tests
- [ ] Place limit order (testnet)
- [ ] Place market order (testnet)
- [ ] Cancel order
- [ ] Get order status

---

## Phase 4.5: Position Tracking
**Status**: PENDING  
**Objective**: Track positions from exchange updates

### Tasks
- [ ] Design `Position` struct
- [ ] Track size, entry price, unrealized PnL
- [ ] Update from WebSocket user data stream
- [ ] Handle partial fills
- [ ] Position reconciliation

### Interface
```rust
#[derive(Copy, Clone)]
pub struct Position {
    pub symbol: Symbol,
    pub side: PositionSide, // Long/Short
    pub size: FixedPoint8,  // Absolute value
    pub entry_price: FixedPoint8,
    pub unrealized_pnl: FixedPoint8,
    pub leverage: u8,
    pub liquidation_price: FixedPoint8,
}

pub struct PositionTracker {
    positions: [Option<Position>; MAX_SYMBOLS],
}

impl PositionTracker {
    pub fn update_from_fill(&mut self, 
        fill: &FillData
    ) -> Option<PositionUpdate>;
    
    pub fn get_position(&self, 
        symbol: Symbol
    ) -> Option<&Position>;
}
```

### HFT Checklist
- [ ] Array-based lookup (no HashMap)
- [ ] Copy type for Position
- [ ] Atomic updates

### Tests
- [ ] Position open
- [ ] Position increase
- [ ] Position decrease
- [ ] Position close
- [ ] Partial fill handling

---

## Phase 4.6: Account State
**Status**: PENDING  
**Objective**: Track balances and margin

### Tasks
- [ ] Design `AccountState` struct
- [ ] Track wallet balance, available margin
- [ ] Handle balance updates from WebSocket
- [ ] Calculate required margin for orders
- [ ] Margin call detection

### Interface
```rustn#[derive(Copy, Clone)]
pub struct AccountState {
    pub wallet_balance: FixedPoint8,
    pub available_balance: FixedPoint8,
    pub margin_balance: FixedPoint8,
    pub unrealized_pnl: FixedPoint8,
    pub maintenance_margin: FixedPoint8,
}

pub struct AccountManager {
    state: AccountState,
    pending_orders_margin: FixedPoint8,
}

impl AccountManager {
    pub fn can_place_order(&self, 
        order: &Order
    ) -> bool;
    
    pub fn update_from_balance(
        &mut self, 
        update: &BalanceUpdate
    );
}
```

### HFT Checklist
- [ ] Lock-free updates
- [ ] Fast balance checks

### Tests
- [ ] Balance update
- [ ] Margin calculation
- [ ] Insufficient funds detection

---

## Sprint 4 Completion Criteria
- [ ] Place order latency <50ms (roundtrip)
- [ ] Cancel order latency <50ms
- [ ] Position tracking accurate
- [ ] Account balance tracking accurate
- [ ] Handle 100 orders/minute per exchange
- [ ] Testnet testing complete
