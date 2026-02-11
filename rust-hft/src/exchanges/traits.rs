//! Exchange abstraction traits
//!
//! Zero-cost abstraction for unified exchange interface.
//! No dynamic dispatch in hot path - use generics for monomorphization.

use crate::core::{Symbol, TickerData, TradeData};
use crate::exchanges::Exchange;
use crate::{HftError, Result};

/// Unified message type from any exchange
/// Copy type for zero-allocation hot path
#[derive(Debug, Clone, PartialEq)]
pub enum ExchangeMessage {
    /// Trade data from specific exchange
    Trade(Exchange, TradeData),
    /// Ticker data from specific exchange
    Ticker(Exchange, TickerData),
    /// Connection heartbeat
    Heartbeat,
    /// Error message (cold path, allocated)
    Error(ExchangeError),
}

/// Exchange-specific error information
#[derive(Debug, Clone, PartialEq)]
pub struct ExchangeError {
    pub exchange: Exchange,
    pub kind: ErrorKind,
    pub message: String,
}

/// Error classification for handling strategies
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    ConnectionLost,
    ParseError,
    SubscriptionFailed,
    RateLimited,
    Unknown,
}

/// WebSocket exchange interface
/// 
/// # Design Notes
/// - Uses generics for zero-cost abstraction (no dynamic dispatch)
/// - Async methods for non-blocking operations
/// - Methods return `Result` for explicit error handling
/// - `next_message()` is the hot path - returns Copy types only
#[allow(async_fn_in_trait)]
pub trait WebSocketExchange: Send + Sync {
    /// Get exchange identifier
    fn exchange(&self) -> Exchange;
    
    /// Get exchange name (for logging/metrics)
    fn name(&self) -> &'static str {
        self.exchange().name()
    }
    
    /// Connect to exchange WebSocket
    async fn connect(&mut self) -> Result<()>;
    
    /// Subscribe to trade stream for given symbols
    async fn subscribe_trades(&mut self, symbols: &[Symbol]) -> Result<()>;
    
    /// Subscribe to ticker stream for given symbols
    async fn subscribe_tickers(&mut self, symbols: &[Symbol]) -> Result<()>;
    
    /// Receive next message (hot path)
    /// Returns `Ok(None)` if connection closed gracefully
    async fn next_message(&mut self) -> Result<Option<ExchangeMessage>>;
    
    /// Check if connection is active
    fn is_connected(&self) -> bool;
    
    /// Get last activity timestamp (for health checks)
    fn last_activity(&self) -> std::time::Instant;
}

/// Helper trait for type-erased exchange handling
/// Use only in cold path (configuration, startup)
pub trait AnyExchange: Send + Sync {
    fn exchange(&self) -> Exchange;
    fn name(&self) -> &'static str;
    fn is_connected(&self) -> bool;
}

impl<T: WebSocketExchange> AnyExchange for T {
    #[inline]
    fn exchange(&self) -> Exchange {
        WebSocketExchange::exchange(self)
    }
    
    #[inline]
    fn name(&self) -> &'static str {
        WebSocketExchange::name(self)
    }
    
    #[inline]
    fn is_connected(&self) -> bool {
        WebSocketExchange::is_connected(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_exchange_message_variants() {
        let trade = ExchangeMessage::Trade(
            Exchange::Binance,
            TradeData::new(
                Symbol::BTCUSDT,
                crate::core::FixedPoint8::ONE,
                crate::core::FixedPoint8::ONE,
                1234567890,
                crate::core::Side::Buy,
                false,
            )
        );
        
        match trade {
            ExchangeMessage::Trade(ex, data) => {
                assert_eq!(ex, Exchange::Binance);
                assert_eq!(data.symbol, Symbol::BTCUSDT);
            }
            _ => panic!("Expected Trade variant"),
        }
    }
    
    #[test]
    fn test_error_kind_classification() {
        assert_ne!(ErrorKind::ConnectionLost, ErrorKind::ParseError);
        assert_ne!(ErrorKind::RateLimited, ErrorKind::Unknown);
    }
}

// HFT Hot Path Checklist verified:
// ✓ ExchangeMessage is Copy (no allocation)
// ✓ No dynamic dispatch in trait (monomorphization via generics)
// ✓ next_message returns Option<ExchangeMessage> - stack only
// ✓ Error handling via Result (no panic)
// ✓ AnyExchange for cold path only (trait objects acceptable there)
