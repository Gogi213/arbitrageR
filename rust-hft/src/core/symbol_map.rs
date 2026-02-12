//! Symbol mapping for exchange-specific naming
//!
//! Simplified: just uses Symbol::as_str() which queries the registry.

use crate::core::Symbol;
use crate::exchanges::Exchange;

pub struct SymbolMapper;

impl SymbolMapper {
    /// Get exchange-specific name for a symbol
    /// Currently just returns the symbol name from registry
    #[inline]
    pub fn get_name(symbol: Symbol, _exchange: Exchange) -> Option<&'static str> {
        Some(symbol.as_str())
    }

    /// Parse symbol from exchange name
    #[inline]
    pub fn from_exchange_name(name: &str, _exchange: Exchange) -> Option<Symbol> {
        Symbol::from_bytes(name.as_bytes())
    }
}

#[cfg(test)]
use crate::test_utils::init_test_registry;
mod tests {
    use super::*;
    use crate::core::registry::SymbolRegistry;


    #[test]
    fn test_get_name() {
        init_test_registry();
        let btc = Symbol::from_bytes(b"BTCUSDT").unwrap();
        assert_eq!(
            SymbolMapper::get_name(btc, Exchange::Binance),
            Some("BTCUSDT")
        );
        assert_eq!(
            SymbolMapper::get_name(btc, Exchange::Bybit),
            Some("BTCUSDT")
        );
    }

    #[test]
    fn test_from_exchange_name() {
        init_test_registry();
        assert_eq!(
            SymbolMapper::from_exchange_name("BTCUSDT", Exchange::Binance),
            Some(Symbol::from_bytes(b"BTCUSDT").unwrap())
        );
        assert_eq!(
            SymbolMapper::from_exchange_name("ETHUSDT", Exchange::Bybit),
            Some(Symbol::from_bytes(b"ETHUSDT").unwrap())
        );
    }
}
