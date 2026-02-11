//! Market data types
//!
//! TickerData and TradeData are core structures for market data.
//! Optimized for cache-line alignment (64 bytes).

use super::{FixedPoint8, Symbol};

/// Best bid/ask ticker data
#[repr(C, align(64))]
#[derive(Debug, Clone, Copy)]
pub struct TickerData {
    /// Trading pair symbol
    pub symbol: Symbol,
    /// Best bid price
    pub bid_price: FixedPoint8,
    /// Best bid quantity
    pub bid_qty: FixedPoint8,
    /// Best ask price
    pub ask_price: FixedPoint8,
    /// Best ask quantity
    pub ask_qty: FixedPoint8,
    /// Timestamp (nanoseconds since epoch)
    pub timestamp: u64,
}

/// Trade side
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Side {
    Buy = 1,
    Sell = 2,
}

impl Side {
    /// Parse side from string (Buy/Sell) or byte values
    #[inline]
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        match bytes {
            b"BUY" | b"buy" | b"Buy" => Some(Self::Buy),
            b"SELL" | b"sell" | b"Sell" => Some(Self::Sell),
            _ => None,
        }
    }

    /// Returns true if Buy
    #[inline(always)]
    pub const fn is_buy(&self) -> bool {
        matches!(self, Self::Buy)
    }

    /// Returns true if Sell
    #[inline(always)]
    pub const fn is_sell(&self) -> bool {
        matches!(self, Self::Sell)
    }
}

/// Individual trade data (aggTrade)
#[repr(C, align(64))]
#[derive(Debug, Clone, Copy)]
pub struct TradeData {
    /// Trading pair symbol
    pub symbol: Symbol,
    /// Trade price
    pub price: FixedPoint8,
    /// Trade quantity
    pub quantity: FixedPoint8,
    /// Timestamp (nanoseconds since epoch)
    pub timestamp: u64,
    /// Trade side
    pub side: Side,
    /// Is buyer maker (true = limit order, false = market order)
    pub is_buyer_maker: bool,
}

impl TickerData {
    /// Create new ticker data
    #[inline(always)]
    pub const fn new(
        symbol: Symbol,
        bid_price: FixedPoint8,
        bid_qty: FixedPoint8,
        ask_price: FixedPoint8,
        ask_qty: FixedPoint8,
        timestamp: u64,
    ) -> Self {
        Self {
            symbol,
            bid_price,
            bid_qty,
            ask_price,
            ask_qty,
            timestamp,
        }
    }

    /// Calculate spread as FixedPoint8 (ask - bid)
    /// Returns None if subtraction overflows
    #[inline]
    pub fn spread(&self) -> Option<FixedPoint8> {
        self.ask_price.checked_sub(self.bid_price)
    }

    /// Calculate spread in basis points
    /// Returns spread * 10000 (so 1% = 100 bps)
    #[inline]
    pub fn spread_bps(&self) -> Option<FixedPoint8> {
        self.bid_price.spread_bps(self.ask_price)
    }

    /// Get mid price (average of bid and ask)
    #[inline]
    pub fn mid_price(&self) -> Option<FixedPoint8> {
        let sum = self.bid_price.checked_add(self.ask_price)?;
        // Divide by 2
        Some(FixedPoint8::from_raw(sum.as_raw() / 2))
    }

    /// Check if this is a valid quote (bid < ask)
    #[inline]
    pub fn is_valid(&self) -> bool {
        self.bid_price.as_raw() < self.ask_price.as_raw()
    }
}

impl TradeData {
    /// Create new trade data
    #[inline(always)]
    pub const fn new(
        symbol: Symbol,
        price: FixedPoint8,
        quantity: FixedPoint8,
        timestamp: u64,
        side: Side,
        is_buyer_maker: bool,
    ) -> Self {
        Self {
            symbol,
            price,
            quantity,
            timestamp,
            side,
            is_buyer_maker,
        }
    }

    /// Calculate notional value (price * quantity)
    #[inline]
    pub fn notional(&self) -> Option<FixedPoint8> {
        self.price.safe_mul(self.quantity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ticker_data_size() {
        // TickerData should be exactly 64 bytes (one cache line)
        assert_eq!(std::mem::size_of::<TickerData>(), 64);
        assert_eq!(std::mem::align_of::<TickerData>(), 64);
    }

    #[test]
    fn test_trade_data_size() {
        // TradeData should be exactly 64 bytes
        assert_eq!(std::mem::size_of::<TradeData>(), 64);
        assert_eq!(std::mem::align_of::<TradeData>(), 64);
    }

    #[test]
    fn test_ticker_creation() {
        let ticker = TickerData::new(
            Symbol::BTCUSDT,
            FixedPoint8::from_raw(99_000_000_00), // 99.0
            FixedPoint8::from_raw(1_000_000_00),  // 1.0
            FixedPoint8::from_raw(101_000_000_00), // 101.0
            FixedPoint8::from_raw(2_000_000_00),  // 2.0
            1234567890,
        );

        assert_eq!(ticker.symbol, Symbol::BTCUSDT);
        assert_eq!(ticker.bid_price.as_raw(), 99_000_000_00);
        assert_eq!(ticker.ask_price.as_raw(), 101_000_000_00);
        assert!(ticker.is_valid());
    }

    #[test]
    fn test_ticker_spread() {
        let ticker = TickerData::new(
            Symbol::BTCUSDT,
            FixedPoint8::from_raw(100_000_000_00), // 100.0
            FixedPoint8::ONE,
            FixedPoint8::from_raw(101_000_000_00), // 101.0
            FixedPoint8::ONE,
            1234567890,
        );

        let spread = ticker.spread().unwrap();
        assert_eq!(spread.as_raw(), 1_000_000_00); // 1.0
    }

    #[test]
    fn test_ticker_mid_price() {
        let ticker = TickerData::new(
            Symbol::BTCUSDT,
            FixedPoint8::from_raw(100_000_000_00), // 100.0
            FixedPoint8::ONE,
            FixedPoint8::from_raw(102_000_000_00), // 102.0
            FixedPoint8::ONE,
            1234567890,
        );

        let mid = ticker.mid_price().unwrap();
        assert_eq!(mid.as_raw(), 101_000_000_00); // 101.0
    }

    #[test]
    fn test_ticker_invalid() {
        let ticker = TickerData::new(
            Symbol::BTCUSDT,
            FixedPoint8::from_raw(101_000_000_00), // 101.0
            FixedPoint8::ONE,
            FixedPoint8::from_raw(100_000_000_00), // 100.0 (crossed!)
            FixedPoint8::ONE,
            1234567890,
        );

        assert!(!ticker.is_valid());
    }

    #[test]
    fn test_trade_creation() {
        let trade = TradeData::new(
            Symbol::ETHUSDT,
            FixedPoint8::from_raw(2000_000_000_00), // 2000.0
            FixedPoint8::from_raw(5_000_000_00),    // 5.0
            1234567890,
            Side::Buy,
            false,
        );

        assert_eq!(trade.symbol, Symbol::ETHUSDT);
        assert_eq!(trade.side, Side::Buy);
        assert!(!trade.is_buyer_maker);
    }

    #[test]
    fn test_trade_notional() {
        let trade = TradeData::new(
            Symbol::BTCUSDT,
            FixedPoint8::from_raw(100_000_000_00), // 100.0
            FixedPoint8::from_raw(2_000_000_00),   // 2.0
            1234567890,
            Side::Sell,
            true,
        );

        let notional = trade.notional().unwrap();
        // 100.0 * 2.0 = 200.0
        assert_eq!(notional.as_raw(), 200_000_000_00);
    }

    #[test]
    fn test_side_parsing() {
        assert_eq!(Side::from_bytes(b"BUY"), Some(Side::Buy));
        assert_eq!(Side::from_bytes(b"SELL"), Some(Side::Sell));
        assert_eq!(Side::from_bytes(b"buy"), Some(Side::Buy));
        assert_eq!(Side::from_bytes(b"sell"), Some(Side::Sell));
        assert_eq!(Side::from_bytes(b"unknown"), None);
    }

    #[test]
    fn test_copy_types() {
        let ticker = TickerData::new(
            Symbol::BTCUSDT,
            FixedPoint8::ONE,
            FixedPoint8::ONE,
            FixedPoint8::from_raw(2_000_000_00),
            FixedPoint8::ONE,
            1234567890,
        );

        let ticker2 = ticker; // Copy
        let ticker3 = ticker; // Can still use ticker

        assert_eq!(ticker.symbol, ticker2.symbol);
        assert_eq!(ticker.symbol, ticker3.symbol);
    }

    #[test]
    fn test_trade_copy() {
        let trade = TradeData::new(
            Symbol::BTCUSDT,
            FixedPoint8::ONE,
            FixedPoint8::ONE,
            1234567890,
            Side::Buy,
            false,
        );

        let trade2 = trade;
        let _trade3 = trade;

        assert_eq!(trade.price, trade2.price);
    }
}

// HFT Hot Path Checklist verified:
// ✓ No heap allocations (Copy types only)
// ✓ No panics (all operations return Option or use checked math)
// ✓ Cache-line aligned (64 bytes exactly)
// ✓ Stack only (no Box, no Vec, no String)
// ✓ SIMD-friendly (can operate on raw bytes)
