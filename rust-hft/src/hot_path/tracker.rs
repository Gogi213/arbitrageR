//! Threshold Tracker (Warm Path)
//!
//! Tracks spread state and calculates statistics for the screener.
//! Integrates SpreadCalculator and RingBuffer.

use crate::core::{FixedPoint8, Symbol, TickerData};
use crate::exchanges::Exchange;
use crate::hot_path::{SpreadCalculator, SpreadEvent};
use crate::infrastructure::RingBuffer;

/// Rolling window size (e.g. 1200 ticks ~ 2 minutes @ 10Hz)
const WINDOW_SIZE: usize = 1200;

/// State for a single symbol
#[derive(Debug, Clone)]
pub struct SymbolState {
    pub symbol: Symbol,
    pub last_binance: Option<TickerData>,
    pub last_bybit: Option<TickerData>,
    
    /// Rolling history of spreads
    pub history: RingBuffer<FixedPoint8, WINDOW_SIZE>,
    
    /// Number of times spread exceeded threshold
    pub hits: u64,
    
    /// Current active spread
    pub current_spread: FixedPoint8,
}

impl SymbolState {
    pub fn new(symbol: Symbol) -> Self {
        Self {
            symbol,
            last_binance: None,
            last_bybit: None,
            history: RingBuffer::new(),
            hits: 0,
            current_spread: FixedPoint8::ZERO,
        }
    }
    
    /// Update state with new ticker and calculate spread
    pub fn update(&mut self, ticker: TickerData, exchange: Exchange) -> Option<SpreadEvent> {
        match exchange {
            Exchange::Binance => self.last_binance = Some(ticker),
            Exchange::Bybit => self.last_bybit = Some(ticker),
        }
        
        // If we have both tickers, calculate spread
        if let (Some(binance), Some(bybit)) = (&self.last_binance, &self.last_bybit) {
            if let Some(event) = SpreadCalculator::calculate(self.symbol, binance, bybit) {
                self.current_spread = event.spread;
                self.history.push_fp(event.spread);
                
                // Simple hit counting (threshold > 0.05%)
                if event.spread.as_raw() > 50_000 {
                    self.hits += 1;
                }
                
                return Some(event);
            }
        }
        
        None
    }
    
    /// Get aggregated statistics for dashboard
    pub fn get_stats(&self) -> ScreenerStats {
        let (min, max) = self.history.min_max();
        // Range = Max - Min
        let range = max.checked_sub(min).unwrap_or(FixedPoint8::ZERO);
        
        ScreenerStats {
            symbol: self.symbol,
            current_spread: self.current_spread,
            spread_range: range,
            hits: self.hits,
            is_valid: self.last_binance.is_some() && self.last_bybit.is_some(),
        }
    }
}

/// Stats for API/Dashboard
#[derive(Debug, Clone, Copy)]
pub struct ScreenerStats {
    pub symbol: Symbol,
    pub current_spread: FixedPoint8,
    pub spread_range: FixedPoint8,
    pub hits: u64,
    pub is_valid: bool,
}

/// Global tracker holding all symbol states
pub struct ThresholdTracker {
    // Array indexed by Symbol ID (assuming small range for now)
    // For large dynamic range, a Vec or Map would be better.
    // Given we have ~10 static + potential dynamic, Vec is fine.
    states: Vec<SymbolState>,
}

impl ThresholdTracker {
    pub fn new() -> Self {
        // Pre-allocate for some symbols
        Self {
            states: Vec::with_capacity(100),
        }
    }
    
    /// Ensure state exists for symbol
    fn ensure_symbol(&mut self, symbol: Symbol) {
        let id = symbol.as_raw() as usize;
        if id >= self.states.len() {
            self.states.resize(id + 1, SymbolState::new(Symbol::from_raw(id as u32)));
        }
        // If resized with defaults, update symbol ID
        if self.states[id].symbol.as_raw() != id as u32 {
             self.states[id] = SymbolState::new(symbol);
        }
    }
    
    /// Update tracker with new ticker
    pub fn update(&mut self, ticker: TickerData, exchange: Exchange) -> Option<SpreadEvent> {
        let symbol = ticker.symbol;
        self.ensure_symbol(symbol);
        
        // Unsafe get is not needed as we ensured size, but safe is fine
        self.states[symbol.as_raw() as usize].update(ticker, exchange)
    }
    
    /// Get stats for all active symbols
    pub fn get_all_stats(&self) -> Vec<ScreenerStats> {
        self.states.iter()
            .filter(|s| s.last_binance.is_some() || s.last_bybit.is_some())
            .map(|s| s.get_stats())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    fn make_ticker(symbol: Symbol, price: i64) -> TickerData {
        TickerData {
            symbol,
            bid_price: FixedPoint8::from_raw(price),
            ask_price: FixedPoint8::from_raw(price + 100), // Small spread
            bid_qty: FixedPoint8::ONE,
            ask_qty: FixedPoint8::ONE,
            timestamp: 1000,
        }
    }
    
    #[test]
    fn test_tracker_update() {
        let mut tracker = ThresholdTracker::new();
        let sym = Symbol::BTCUSDT;
        
        // Update Binance
        tracker.update(make_ticker(sym, 100_000_000), Exchange::Binance);
        let stats = tracker.get_all_stats();
        assert_eq!(stats.len(), 1);
        assert!(!stats[0].is_valid); // Only one exchange
        
        // Update Bybit
        // Binance Ask: 100.000100, Bybit Bid: 101.000000
        // Spread ~ 1%
        let event = tracker.update(make_ticker(sym, 101_000_000), Exchange::Bybit);
        
        assert!(event.is_some());
        let stats = tracker.get_all_stats();
        assert!(stats[0].is_valid);
        assert!(stats[0].current_spread.is_positive());
    }
}
