//! Hot path spread calculation
//!
//! Zero-allocation implementation of arbitrage spread calculation.
//! Uses FixedPoint8 for precision and speed.

use crate::core::{FixedPoint8, Symbol, TickerData};
use crate::exchanges::Exchange;

/// Spread calculation result
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpreadEvent {
    pub symbol: Symbol,
    /// Spread value (bps or percentage)
    pub spread: FixedPoint8,
    /// Exchange to Buy on
    pub long_ex: Exchange,
    /// Exchange to Sell on
    pub short_ex: Exchange,
    /// Timestamp (max of both tickers)
    pub timestamp: u64,
}

/// Zero-allocation spread calculator
pub struct SpreadCalculator;

impl SpreadCalculator {
    /// Calculate spread between two tickers
    /// 
    /// Formula: (Bid_Short - Ask_Long) / Ask_Long
    /// Returns the best spread opportunity (Long A/Short B or Long B/Short A)
    #[inline]
    pub fn calculate(
        symbol: Symbol,
        binance: &TickerData,
        bybit: &TickerData
    ) -> Option<SpreadEvent> {
        // Validate symbols match
        // In hot path we assume caller checked this, but debug assert helps
        debug_assert_eq!(binance.symbol, symbol);
        debug_assert_eq!(bybit.symbol, symbol);
        debug_assert_eq!(binance.symbol, bybit.symbol);

        // 1. Check Long Binance (Buy) / Short Bybit (Sell)
        // Profit = (Bybit Bid - Binance Ask) / Binance Ask
        // We want to buy low (Ask) and sell high (Bid)
        let spread_long_binance = if binance.ask_price.is_positive() {
             bybit.bid_price
                .checked_sub(binance.ask_price)
                .and_then(|diff| diff.safe_div(binance.ask_price))
                .unwrap_or(FixedPoint8::ZERO)
        } else {
            FixedPoint8::ZERO
        };

        // 2. Check Long Bybit (Buy) / Short Binance (Sell)
        // Profit = (Binance Bid - Bybit Ask) / Bybit Ask
        let spread_long_bybit = if bybit.ask_price.is_positive() {
            binance.bid_price
                .checked_sub(bybit.ask_price)
                .and_then(|diff| diff.safe_div(bybit.ask_price))
                .unwrap_or(FixedPoint8::ZERO)
        } else {
            FixedPoint8::ZERO
        };

        // Select better spread
        if spread_long_binance > spread_long_bybit {
            Some(SpreadEvent {
                symbol,
                spread: spread_long_binance,
                long_ex: Exchange::Binance,
                short_ex: Exchange::Bybit,
                timestamp: std::cmp::max(binance.timestamp, bybit.timestamp),
            })
        } else {
            Some(SpreadEvent {
                symbol,
                spread: spread_long_bybit,
                long_ex: Exchange::Bybit,
                short_ex: Exchange::Binance,
                timestamp: std::cmp::max(binance.timestamp, bybit.timestamp),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::FixedPoint8;

    fn make_ticker(bid: i64, ask: i64) -> TickerData {
        TickerData {
            symbol: Symbol::BTCUSDT,
            bid_price: FixedPoint8::from_raw(bid * FixedPoint8::SCALE),
            ask_price: FixedPoint8::from_raw(ask * FixedPoint8::SCALE),
            bid_qty: FixedPoint8::ONE,
            ask_qty: FixedPoint8::ONE,
            timestamp: 1000,
        }
    }

    #[test]
    fn test_spread_long_binance() {
        // Binance: Buy at 100 (Ask)
        // Bybit: Sell at 101 (Bid)
        // Spread = (101 - 100) / 100 = 0.01 (1%)
        let binance = make_ticker(99, 100);
        let bybit = make_ticker(101, 102);

        let event = SpreadCalculator::calculate(Symbol::BTCUSDT, &binance, &bybit).unwrap();
        
        assert_eq!(event.long_ex, Exchange::Binance);
        assert_eq!(event.short_ex, Exchange::Bybit);
        assert_eq!(event.spread, FixedPoint8::from_raw(1_000_000)); // 0.01 * 10^8
    }

    #[test]
    fn test_spread_long_bybit() {
        // Binance: Sell at 101 (Bid)
        // Bybit: Buy at 100 (Ask)
        // Spread = (101 - 100) / 100 = 0.01
        let binance = make_ticker(101, 102);
        let bybit = make_ticker(99, 100);

        let event = SpreadCalculator::calculate(Symbol::BTCUSDT, &binance, &bybit).unwrap();
        
        assert_eq!(event.long_ex, Exchange::Bybit);
        assert_eq!(event.short_ex, Exchange::Binance);
        assert_eq!(event.spread, FixedPoint8::from_raw(1_000_000));
    }
    
    #[test]
    fn test_negative_spread() {
        // Binance: 100/101
        // Bybit: 100/101
        // No arb: (100 - 101) / 101 = -0.0099
        let binance = make_ticker(100, 101);
        let bybit = make_ticker(100, 101);
        
        let event = SpreadCalculator::calculate(Symbol::BTCUSDT, &binance, &bybit).unwrap();
        assert!(event.spread.is_negative());
    }
}

// HFT Hot Path Checklist verified:
// ✓ No heap allocations
// ✓ Stack-only returns (Option<SpreadEvent>)
// ✓ FixedPoint8 arithmetic (no floats)
// ✓ Branchless potential (if/else is predictable)
