//! Message router for zero-allocation dispatch
//!
//! Routes incoming market data messages to handlers using array-based lookup.
//! O(1) performance with no HashMap and no allocation in hot path.

use crate::core::{Symbol, TickerData, TradeData};

/// Maximum number of symbols that can be registered
/// Should match Symbol::MAX_SYMBOLS
pub const MAX_ROUTES: usize = 10_000;

/// Handler function type for ticker data
pub type TickerHandler = fn(symbol: Symbol, data: TickerData);

/// Handler function type for trade data  
pub type TradeHandler = fn(symbol: Symbol, data: TradeData);

/// Message router with array-based dispatch
///
/// Uses direct array indexing by Symbol ID for O(1) lookup.
/// No HashMap, no allocation in hot path, arrays boxed to heap to avoid stack overflow.
pub struct MessageRouter {
    /// Handlers for ticker data (indexed by Symbol ID, boxed to heap)
    ticker_handlers: Box<[Option<TickerHandler>; MAX_ROUTES]>,
    /// Handlers for trade data (indexed by Symbol ID, boxed to heap)
    trade_handlers: Box<[Option<TradeHandler>; MAX_ROUTES]>,
    /// Fallback handler for unregistered symbols (cold path)
    fallback_ticker_handler: Option<TickerHandler>,
    /// Fallback handler for unregistered trade symbols (cold path)
    fallback_trade_handler: Option<TradeHandler>,
    /// Number of registered routes (for stats)
    registered_count: usize,
}

impl MessageRouter {
    /// Create new message router with empty handlers
    ///
    /// Arrays are boxed to heap to avoid stack overflow with large MAX_ROUTES.
    pub fn new() -> Self {
        Self {
            // Initialize with None - boxed to heap to avoid stack overflow
            ticker_handlers: Box::new([None; MAX_ROUTES]),
            trade_handlers: Box::new([None; MAX_ROUTES]),
            fallback_ticker_handler: None,
            fallback_trade_handler: None,
            registered_count: 0,
        }
    }

    /// Register a ticker handler for a symbol
    ///
    /// # Arguments
    /// * `symbol` - The trading pair symbol
    /// * `handler` - Function to call when ticker data arrives
    ///
    /// # Example
    /// ```
    /// router.register_ticker(Symbol::from_bytes(b"BTCUSDT").unwrap(), |sym, data| {
    ///     println!("Ticker for {:?}: bid={}, ask={}", sym, data.bid_price, data.ask_price);
    /// });
    /// ```
    pub fn register_ticker(&mut self, symbol: Symbol, handler: TickerHandler) {
        let idx = symbol.as_raw() as usize;
        if idx < MAX_ROUTES {
            if self.ticker_handlers[idx].is_none() {
                self.registered_count += 1;
            }
            self.ticker_handlers[idx] = Some(handler);
        }
    }

    /// Register a trade handler for a symbol
    pub fn register_trade(&mut self, symbol: Symbol, handler: TradeHandler) {
        let idx = symbol.as_raw() as usize;
        if idx < MAX_ROUTES {
            if self.trade_handlers[idx].is_none() {
                self.registered_count += 1;
            }
            self.trade_handlers[idx] = Some(handler);
        }
    }

    /// Route ticker data to the appropriate handler
    ///
    /// # Hot Path
    /// This is called on every ticker update - must be extremely fast.
    /// Uses unsafe get_unchecked for zero-cost bounds checking.
    #[inline(always)]
    pub fn route_ticker(&self, symbol: Symbol, data: TickerData) {
        let idx = symbol.as_raw() as usize;

        // Safety: Symbol ID is always < MAX_ROUTES (enforced by Symbol type)
        // This avoids bounds check in hot path
        unsafe {
            if let Some(handler) = self.ticker_handlers.get_unchecked(idx) {
                handler(symbol, data);
            } else if let Some(fallback) = self.fallback_ticker_handler {
                fallback(symbol, data);
            }
        }
    }

    /// Route trade data to the appropriate handler
    #[inline(always)]
    pub fn route_trade(&self, symbol: Symbol, data: TradeData) {
        let idx = symbol.as_raw() as usize;

        unsafe {
            if let Some(handler) = self.trade_handlers.get_unchecked(idx) {
                handler(symbol, data);
            } else if let Some(fallback) = self.fallback_trade_handler {
                fallback(symbol, data);
            }
        }
    }

    /// Set fallback handler for unregistered ticker symbols
    pub fn set_fallback_ticker(&mut self, handler: TickerHandler) {
        self.fallback_ticker_handler = Some(handler);
    }

    /// Set fallback handler for unregistered trade symbols
    pub fn set_fallback_trade(&mut self, handler: TradeHandler) {
        self.fallback_trade_handler = Some(handler);
    }

    /// Get the number of registered handlers
    #[inline(always)]
    pub fn registered_count(&self) -> usize {
        self.registered_count
    }

    /// Check if a symbol has a ticker handler registered
    pub fn has_ticker_handler(&self, symbol: Symbol) -> bool {
        let idx = symbol.as_raw() as usize;
        idx < MAX_ROUTES && self.ticker_handlers[idx].is_some()
    }

    /// Check if a symbol has a trade handler registered
    pub fn has_trade_handler(&self, symbol: Symbol) -> bool {
        let idx = symbol.as_raw() as usize;
        idx < MAX_ROUTES && self.trade_handlers[idx].is_some()
    }

    /// Unregister a ticker handler
    pub fn unregister_ticker(&mut self, symbol: Symbol) {
        let idx = symbol.as_raw() as usize;
        if idx < MAX_ROUTES && self.ticker_handlers[idx].is_some() {
            self.ticker_handlers[idx] = None;
            self.registered_count -= 1;
        }
    }

    /// Unregister a trade handler
    pub fn unregister_trade(&mut self, symbol: Symbol) {
        let idx = symbol.as_raw() as usize;
        if idx < MAX_ROUTES && self.trade_handlers[idx].is_some() {
            self.trade_handlers[idx] = None;
            self.registered_count -= 1;
        }
    }
}

impl Default for MessageRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
use crate::test_utils::init_test_registry;
mod tests {
    use super::*;
    use crate::core::{registry::SymbolRegistry, FixedPoint8};


    fn make_ticker(symbol: Symbol) -> TickerData {
        TickerData {
            symbol,
            bid_price: FixedPoint8::from_raw(100_000_000),
            ask_price: FixedPoint8::from_raw(100_000_100),
            bid_qty: FixedPoint8::ONE,
            ask_qty: FixedPoint8::ONE,
            timestamp: 1000,
        }
    }

    fn make_trade(symbol: Symbol) -> TradeData {
        TradeData {
            symbol,
            price: FixedPoint8::from_raw(100_000_000),
            quantity: FixedPoint8::ONE,
            timestamp: 1000,
            side: crate::core::Side::Buy,
            is_buyer_maker: false,
        }
    }

    #[test]
    fn test_register_and_route_ticker() {
        init_test_registry();
        let mut router = MessageRouter::new();
        let btc = Symbol::from_bytes(b"BTCUSDT").unwrap();

        static CALL_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        fn handler(_sym: Symbol, _data: TickerData) {
            CALL_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }

        router.register_ticker(btc, handler);
        router.route_ticker(btc, make_ticker(btc));
        assert_eq!(CALL_COUNT.load(std::sync::atomic::Ordering::Relaxed), 1);
    }

    #[test]
    fn test_register_and_route_trade() {
        init_test_registry();
        let mut router = MessageRouter::new();
        let eth = Symbol::from_bytes(b"ETHUSDT").unwrap();

        static CALL_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        fn handler(_sym: Symbol, _data: TradeData) {
            CALL_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }

        router.register_trade(eth, handler);
        router.route_trade(eth, make_trade(eth));
        assert_eq!(CALL_COUNT.load(std::sync::atomic::Ordering::Relaxed), 1);
    }

    #[test]
    fn test_unregistered_symbol() {
        init_test_registry();
        let router = MessageRouter::new();
        let btc = Symbol::from_bytes(b"BTCUSDT").unwrap();

        static CALL_COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        fn handler(_sym: Symbol, _data: TickerData) {
            CALL_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }

        router.route_ticker(btc, make_ticker(btc));
        assert_eq!(CALL_COUNT.load(std::sync::atomic::Ordering::Relaxed), 0);
    }

    #[test]
    fn test_registered_count() {
        init_test_registry();
        let mut router = MessageRouter::new();
        let btc = Symbol::from_bytes(b"BTCUSDT").unwrap();
        let eth = Symbol::from_bytes(b"ETHUSDT").unwrap();

        fn ticker_handler(_sym: Symbol, _data: TickerData) {}
        fn trade_handler(_sym: Symbol, _data: TradeData) {}

        assert_eq!(router.registered_count(), 0);
        router.register_ticker(btc, ticker_handler);
        assert_eq!(router.registered_count(), 1);
        router.register_ticker(eth, ticker_handler);
        assert_eq!(router.registered_count(), 2);
        router.register_trade(btc, trade_handler);
        assert_eq!(router.registered_count(), 3);
    }
}

// HFT Hot Path Checklist verified:
// ✓ No HashMap (array lookup only)
// ✓ No allocation in route()
// ✓ No bounds check (unsafe get_unchecked)
// ✓ Handler is fn pointer (no dyn Trait)
// ✓ O(1) lookup via array index
// ✓ No string operations
// ✓ Copy types only in hot path
