# Sprint 2: WebSocket Infrastructure

**Goal**: Ultra-low latency WebSocket connections with zero-copy message handling.

## Phase 2.1: WebSocket Connection Core
**Status**: PENDING  
**Objective**: Base WebSocket client with HFT optimizations

### Tasks
- [ ] Wrap `tokio-tungstenite` with custom config
- [ ] Disable per-message deflate (adds latency)
- [ ] Set TCP_NODELAY, SO_RCVBUF, SO_SNDBUF
- [ ] Implement connection state machine
- [ ] Add connection metrics (cold path)
- [ ] Support multiple concurrent connections

### Interface
```rust
pub struct WebSocketConnection {
    stream: WebSocketStream,
    read_buffer: BytesMut,
    // ...
}

impl WebSocketConnection {
    pub async fn connect(url: &str) -> Result<Self>;
    pub async fn send(&mut self, msg: Message) -> Result<()>;
    pub async fn recv(&mut self) -> Result<Option<Message>>;
    pub fn set_read_buffer_capacity(&mut self, size: usize);
}
```

### HFT Checklist
- [ ] Read buffer reused (no alloc per message)
- [ ] Write path: pre-serialized messages
- [ ] No logging in send/recv (cold path only)
- [ ] Fast path: single branch

### Tests
- [ ] Connect to echo server
- [ ] Send/recv 100k messages
- [ ] Reconnection test
- [ ] Latency benchmark

---

## Phase 2.2: Message Router
**Status**: COMPLETE ✅  
**Objective**: Route incoming messages to handlers with zero allocation

### Tasks
- [x] Design `MessageRouter` with pre-registered handlers
- [x] Use symbol ID to index handler table (array, not HashMap)
- [x] Support wildcard handlers (fallback)
- [x] Hot path: single array lookup
- [x] Cold path: dynamic handler registration

### Interface
```rust
pub type TickerHandler = fn(symbol: Symbol, data: TickerData);
pub type TradeHandler = fn(symbol: Symbol, data: TradeData);

pub struct MessageRouter {
    ticker_handlers: [Option<TickerHandler>; MAX_ROUTES],
    trade_handlers: [Option<TradeHandler>; MAX_ROUTES],
    fallback_ticker_handler: Option<TickerHandler>,
    fallback_trade_handler: Option<TradeHandler>,
}

impl MessageRouter {
    pub fn register_ticker(&mut self, symbol: Symbol, handler: TickerHandler);
    pub fn register_trade(&mut self, symbol: Symbol, handler: TradeHandler);
    pub fn set_fallback_ticker(&mut self, handler: TickerHandler);
    
    #[inline(always)]
    pub fn route_ticker(&self, symbol: Symbol, data: TickerData) {
        let idx = symbol.as_raw() as usize;
        unsafe {
            if let Some(handler) = self.ticker_handlers.get_unchecked(idx) {
                handler(symbol, data);
            } else if let Some(fallback) = self.fallback_ticker_handler {
                fallback(symbol, data);
            }
        }
    }
}
```

### HFT Checklist
- [x] No HashMap (O(1) array lookup)
- [x] No allocation in route()
- [x] No bounds check (use get_unchecked)
- [x] Handler is fn pointer (no dyn Trait)

### Tests
- [x] Register and route ticker
- [x] Register and route trade
- [x] Unregistered symbol handling
- [x] Fallback handlers
- [x] Multiple handlers
- [x] Unregistration

### Implementation Notes
- Separate handler arrays for ticker and trade data
- `get_unchecked()` eliminates bounds check in hot path
- Safety: Symbol ID always < MAX_ROUTES (enforced by Symbol type)
- Fallback handlers for unregistered symbols (cold path)
- Registered count tracking for monitoring

### Commit
`007 - Sprint 2: Phase 2.2 Message Router`

---

## Phase 2.3: Connection Pool
**Status**: COMPLETE ✅  
**Objective**: Manage multiple exchange connections

### Tasks
- [x] Design `ConnectionPool` for Binance/Bybit
- [x] Separate connections per data type (trades, orderbook, orders)
- [x] Health monitoring (cold path)
- [x] Automatic reconnection with backoff
- [x] Connection management (add/remove/disconnect)

### Interface
```rust
pub struct ConnectionPool {
    connections: HashMap<ConnectionId, ManagedConnection>,
    next_id: u64,
}

impl ConnectionPool {
    pub fn add_connection(&mut self, config: ConnectionConfig) -> ConnectionId;
    pub async fn connect_all(&mut self) -> Result<()>;
    pub async fn disconnect(&mut self, id: ConnectionId) -> Result<()>;
    pub async fn maintenance(&mut self);
    pub fn stats(&self) -> PoolStats;
}
```

### HFT Checklist
- [x] No allocation in hot path
- [x] Pre-allocated connection tracking
- [x] Fast path: get_connection returns &mut
- [x] Exponential backoff for reconnection
- [x] Health monitoring (idle time tracking)

### Tests
- [x] Pool creation and management
- [x] Connection configuration
- [x] Pool statistics
- [x] Health checks

### Implementation Notes
- Uses HashMap for connection storage (warm/cold path only)
- ManagedConnection tracks metadata (reconnect_count, last_activity)
- Exponential backoff: 1s → 2s → 4s → ... → 60s max
- ConnectionId-based lookup for managing multiple connections
- Stats provide visibility into pool health

### Commit
`008 - Sprint 2: Phase 2.3 Connection Pool`

---

## Phase 2.4: Subscription Manager
**Status**: PENDING  
**Objective**: Batch symbol subscriptions efficiently

### Tasks
- [ ] Batch subscriptions (200 symbols per request for Binance)
- [ ] Manage subscription state
- [ ] Handle subscription confirmations
- [ ] Retry failed subscriptions
- [ ] Track active subscriptions (no duplicates)

### Interface
```rust
pub struct SubscriptionManager {
    pending: Vec<SubscriptionRequest>,
    active: BitSet<MAX_SYMBOLS>,
}

impl SubscriptionManager {
    pub fn request_subscription(&mut self, symbols: &[Symbol]);
    pub fn batch_requests(&mut self, batch_size: usize) -> Vec<SubscriptionRequest>;
    pub fn confirm(&mut self, symbols: &[Symbol]);
}
```

### HFT Checklist
- [ ] BitSet for O(1) active check
- [ ] No allocation in batch creation (pre-allocated)
- [ ] Minimal copying

### Tests
- [ ] Subscribe 500 symbols
- [ ] Confirm batch handling
- [ ] Duplicate subscription prevention

---

## Phase 2.5: Ping/Pong Handler
**Status**: PENDING  
**Objective**: Keep connections alive without blocking hot path

### Tasks
- [ ] Background ping task per connection
- [ ] Track last pong time
- [ ] Detect stale connections
- [ ] Automatic reconnection on timeout
- [ ] Configurable ping interval

### Interface
```rust
pub struct PingHandler {
    interval: Duration,
    timeout: Duration,
    last_pong: Instant,
}

impl PingHandler {
    pub async fn run(&mut self, conn: &mut WebSocketConnection);
    pub fn record_pong(&mut self);
    pub fn is_stale(&self) -> bool;
}
```

### HFT Checklist
- [ ] Ping runs in separate task (not hot path)
- [ ] Atomic/lock-free pong tracking
- [ ] No blocking in ping handler

### Tests
- [ ] Ping sent at interval
- [ ] Stale connection detection
- [ ] Reconnection on timeout

---

## Sprint 2 Completion Criteria
- [ ] All phases complete
- [ ] Connect to Binance & Bybit simultaneously
- [ ] Handle 10k+ messages/second per connection
- [ ] Sub-50μs message processing (excluding parsing)
- [ ] Automatic reconnection working
- [ ] No memory leaks in long-running test
