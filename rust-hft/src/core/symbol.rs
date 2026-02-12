//! Symbol interning for zero-allocation string handling
//!
//! Symbols are stored as u32 IDs with pre-registered lookup.
//! Zero-allocation parsing from JSON byte slices.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Symbol(u32);

impl Symbol {
    pub const MAX_SYMBOLS: u32 = 5000;
    pub const UNKNOWN: Self = Self(u32::MAX);

    #[inline(always)]
    pub const fn from_raw(id: u32) -> Self {
        Self(id)
    }
    #[inline(always)]
    pub const fn as_raw(&self) -> u32 {
        self.0
    }

    #[inline]
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.is_empty() {
            return None;
        }

        // Lookup in registry (single source of truth)
        if let Some(registry) = crate::core::registry::SymbolRegistry::try_global() {
            return registry.lookup(bytes);
        }

        None
    }

    #[inline]
    pub fn as_str(&self) -> &'static str {
        // Lookup in registry (single source of truth)
        if let Some(registry) = crate::core::registry::SymbolRegistry::try_global() {
            if let Some(name) = registry.get_name(*self) {
                return name;
            }
        }
        "UNKNOWN"
    }

    #[inline(always)]
    pub const fn is_valid(&self) -> bool {
        self.0 != Self::UNKNOWN.0
    }
}

impl Default for Symbol {
    #[inline(always)]
    fn default() -> Self {
        Self(0)
    }
}

#[cfg(test)]
use crate::test_utils::init_test_registry;
mod tests {
    use super::*;
    use crate::core::registry::SymbolRegistry;


    #[test]
    fn test_parse_symbols_via_registry() {
        init_test_registry();

        let btc = Symbol::from_bytes(b"BTCUSDT").expect("BTCUSDT should be found");
        let eth = Symbol::from_bytes(b"ETHUSDT").expect("ETHUSDT should be found");

        // Same symbol should return same ID
        let btc2 = Symbol::from_bytes(b"BTCUSDT").expect("BTCUSDT should be found");
        assert_eq!(btc, btc2);
        assert_ne!(btc, eth);
    }

    #[test]
    fn test_as_str_via_registry() {
        init_test_registry();

        let btc = Symbol::from_bytes(b"BTCUSDT").unwrap();
        assert_eq!(btc.as_str(), "BTCUSDT");

        let eth = Symbol::from_bytes(b"ETHUSDT").unwrap();
        assert_eq!(eth.as_str(), "ETHUSDT");
    }

    #[test]
    fn test_invalid_symbol() {
        assert!(Symbol::from_bytes(b"").is_none());
        assert!(Symbol::from_bytes(b"UNKNOWNCOIN").is_none());
    }

    #[test]
    fn test_symbol_comparison() {
        init_test_registry();

        let a = Symbol::from_bytes(b"BTCUSDT").unwrap();
        let b = Symbol::from_bytes(b"BTCUSDT").unwrap();
        let c = Symbol::from_bytes(b"ETHUSDT").unwrap();
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn test_symbol_copy() {
        init_test_registry();

        let a = Symbol::from_bytes(b"BTCUSDT").unwrap();
        let b = a;
        let c = a;
        assert_eq!(a, b);
        assert_eq!(a, c);
    }

    #[test]
    fn test_unknown_symbol() {
        assert!(!Symbol::UNKNOWN.is_valid());

        init_test_registry();
        let btc = Symbol::from_bytes(b"BTCUSDT").unwrap();
        assert!(btc.is_valid());
    }

    #[test]
    fn test_symbol_roundtrip() {
        init_test_registry();

        // Parse -> as_str -> parse should give same symbol
        let sym1 = Symbol::from_bytes(b"BTCUSDT").unwrap();
        let name = sym1.as_str();
        let sym2 = Symbol::from_bytes(name.as_bytes()).unwrap();
        assert_eq!(sym1, sym2);
    }
}
