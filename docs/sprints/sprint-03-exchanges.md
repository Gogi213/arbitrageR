# Sprint 3: Exchange Implementations

**Goal**: Binance and Bybit specific implementations with HFT optimizations.

## Phase 3.1: Binance Native WebSocket
**Status**: PENDING  
**Objective**: HFT-optimized Binance Futures WS client

### Tasks
- [ ] Implement `BinanceWsClient`
- [ ] Connect to `wss://fstream.binance.com/ws`
- [ ] Handle combined streams (@aggTrade, @bookTicker)
- [ ] Parse incoming JSON with zero-copy (simd-json)
- [ ] Route messages by symbol
- [ ] Handle errors gracefully (no panic)

### Interface
```rust
pub struct BinanceWsClient {
    connection: WebSocketConnection,
    parser: BinanceMessageParser,
    router: MessageRouter,
}

impl BinanceWsClient {
    pub async fn connect() -> Result<Self>;
    pub async fn subscribe_agg_trade(&mut self, symbols: &[Symbol]) -> Result<()>;
    pub async fn subscribe_book_ticker(&mut self, symbols: &[Symbol]) -> Result<()>;
    pub async fn run(&mut self) -> Result<()>;
}
```

### Message Parsing
- Use `simd-json` with borrowed strings
- Parse directly into `TradeData` / `TickerData`
- No intermediate allocations
- Branchless symbol lookup

### HFT Checklist
- [ ] No allocation per message
- [ ] No panic in parser (all error handling)
- [ ] Branchless hot path
- [ ] Direct buffer-to-struct parsing

### Tests
- [ ] Parse real Binance messages
- [ ] Handle malformed JSON
- [ ] Subscription confirmation
- [ ] Latency benchmark: parse <1μs

---

## Phase 3.2: Bybit Native WebSocket
**Status**: PENDING  
**Objective**: HFT-optimized Bybit Futures WS client

### Tasks
- [ ] Implement `BybitWsClient`
- [ ] Connect to `wss://stream.bybit.com/v5/public/linear`
- [ ] Handle V5 WebSocket protocol
- [ ] Parse public trade and orderbook messages
- [ ] Implement ping/pong (Bybit specific)
- [ ] Handle topic subscriptions

### Interface
```rust
pub struct BybitWsClient {
    connection: WebSocketConnection,
    parser: BybitMessageParser,
    router: MessageRouter,
}

impl BybitWsClient {
    pub async fn connect() -> Result<Self>;
    pub async fn subscribe_public_trade(&mut self, symbols: &[Symbol]) -> Result<()>;
    pub async fn subscribe_orderbook(&mut self, symbols: &[Symbol], depth: u8) -> Result<()>;
    pub async fn run(&mut self) -> Result<()>;
}
```

### Bybit Specifics
- V5 protocol with topic-based subscriptions
- Different message format than Binance
- Custom ping/pong handling
- Linear (USDT Perp) vs Inverse

### HFT Checklist
- [ ] Same performance as Binance client
- [ ] Zero allocation parsing
- [ ] No panics

### Tests
- [ ] Parse real Bybit messages
- [ ] Topic subscription
- [ ] Ping/pong exchange
- [ ] Latency benchmark

---

## Phase 3.3: Zero-Copy JSON Parsers
**Status**: PENDING  
**Objective**: Custom parsers for exchange message formats

### Tasks
- [ ] Implement `BinanceParser`
- [ ] Implement `BybitParser`
- [ ] Parse without creating intermediate objects
- [ ] Use `serde` with zero-copy where possible
- [ ] Fallback to manual parsing for hot path
- [ ] Handle all message types

### Interface
```rustnpub struct BinanceParser;

impl BinanceParser {
    // Returns parsed data + number of bytes consumed
    pub fn parse_trade(data: &[u8]) -> Option<(TradeData, usize)>;
    pub fn parse_ticker(data: &[u8]) -> Option<(TickerData, usize)>;
}

pub struct BybitParser;

impl BybitParser {
    pub fn parse_public_trade(data: &[u8]) -> Option<(TradeData, usize)>;
    pub fn parse_orderbook(data: &[u8]) -> Option<(OrderBookUpdate, usize)>;
}
```

### Parsing Strategy
- Parse JSON in-place (borrowed strings)
- Convert string prices directly to FixedPoint8
- Symbol lookup via static table
- Timestamp conversion (ms → ns if needed)

### HFT Checklist
- [ ] No allocation during parse
- [ ] No string copies
- [ ] Direct to struct conversion
- [ ] Error handling without panic

### Tests
- [ ] Property-based parsing tests
- [ ] Fuzz test with random JSON
- [ ] Benchmark: <500ns per message

---

## Phase 3.4: Exchange Abstractions
**Status**: PENDING  
**Objective**: Common trait for exchange clients

### Tasks
- [ ] Define `Exchange` trait
- [ ] Define `WebSocketExchange` trait
- [ ] Implement for both Binance and Bybit
- [ ] Ensure zero-cost abstraction
- [ ] Common error types

### Interface
```rust
pub trait WebSocketExchange: Send + Sync {
    fn name(&self) -> &'static str;
    async fn connect(&mut self) -> Result<()>;
    async fn subscribe_trades(&mut self, symbols: &[Symbol]) -> Result<()>;
    async fn subscribe_tickers(&mut self, symbols: &[Symbol]) -> Result<()>;
    async fn next_message(&mut self) -> Result<ExchangeMessage>;
    fn is_connected(&self) -> bool;
}

pub enum ExchangeMessage {
    Trade(Exchange, TradeData),
    Ticker(Exchange, TickerData),
    Heartbeat,
    Error(String),
}
```

### HFT Checklist
- [ ] Trait methods inlined
- [ ] No dynamic dispatch in hot path
- [ ] ExchangeMessage is Copy type

### Tests
- [ ] Trait object vs generic benchmark
- [ ] Verify zero-cost abstraction

---

## Phase 3.5: Symbol Mapping
**Status**: PENDING  
**Objective**: Normalize symbols across exchanges

### Tasks
- [ ] Map Binance symbols (BTCUSDT) to canonical form
- [ ] Map Bybit symbols (BTCUSDT same format)
- [ ] Handle edge cases (1000PEPEUSDT, etc.)
- [ ] Create symbol equivalence table
- [ ] Validate symbol exists on both exchanges

### Interface
```rust
pub struct SymbolMapper {
    binance_to_canonical: [Symbol; MAX_BINANCE_SYMBOLS],
    bybit_to_canonical: [Symbol; MAX_BYBIT_SYMBOLS],
}

impl SymbolMapper {
    pub fn binance_symbol(binance_id: u32) -> Symbol;
    pub fn bybit_symbol(bybit_id: u32) -> Symbol;
    pub fn common_symbols() -> &'static [Symbol];
}
```

### HFT Checklist
- [ ] O(1) lookup via array indexing
- [ ] No string comparison
- [ ] Static initialization

### Tests
- [ ] All common symbols mapped
- [ ] Edge case symbols handled
- [ ] Invalid symbol rejection

---

## Sprint 3 Completion Criteria
- [ ] Both Binance and Bybit clients working
- [ ] Parse >50k messages/second per exchange
- [ ] <2μs average parse time
- [ ] 99.9% uptime in 24h test
- [ ] Memory usage stable (no leaks)
