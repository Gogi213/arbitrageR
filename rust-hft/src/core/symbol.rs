//! Symbol interning for zero-allocation string handling
//!
//! Symbols are stored as u32 IDs with pre-registered lookup.
//! Zero-allocation parsing from JSON byte slices.
//!
//! Architecture:
//! - IDs 0-10: Pre-defined constants (BTCUSDT, ETHUSDT, etc.)
//! - IDs 11+: Dynamically registered at startup via SymbolRegistry
//!
//! Hot Path: from_bytes() does O(1) lookup, no locks, no allocation


/// Trading pair symbol (interned)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Symbol(u32);

impl Symbol {
    /// Maximum number of symbols supported
    pub const MAX_SYMBOLS: u32 = 5000;

    /// Unknown symbol (returned when not found)
    pub const UNKNOWN: Self = Self(u32::MAX);

    /// Create from raw u32 ID
    #[inline(always)]
    pub const fn from_raw(id: u32) -> Self {
        Self(id)
    }

    /// Get raw ID
    #[inline(always)]
    pub const fn as_raw(&self) -> u32 {
        self.0
    }

    /// Parse from byte slice (hot path, lock-free)
    ///
    /// Returns Symbol::UNKNOWN if not registered.
    /// Use SymbolRegistry::initialize() at startup to register symbols.
    #[inline]
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.is_empty() {
            return None;
        }

        // Fast path: check pre-defined symbols with direct comparison
        // These are branch-predictable and avoid hash computation
        match bytes {
            b"BTCUSDT" => return Some(Symbol::BTCUSDT),
            b"ETHUSDT" => return Some(Symbol::ETHUSDT),
            b"SOLUSDT" => return Some(Symbol::SOLUSDT),
            b"BNBUSDT" => return Some(Symbol::BNBUSDT),
            b"XRPUSDT" => return Some(Symbol::XRPUSDT),
            b"ADAUSDT" => return Some(Symbol::ADAUSDT),
            b"DOGEUSDT" => return Some(Symbol::DOGEUSDT),
            b"AVAXUSDT" => return Some(Symbol::AVAXUSDT),
            b"TRXUSDT" => return Some(Symbol::TRXUSDT),
            b"DOTUSDT" => return Some(Symbol::DOTUSDT),
            b"PEPEUSDT" => return Some(Symbol::PEPEUSDT),
            _ => {}
        }

        // Lookup in registry (if initialized)
        // This is still O(1) hash lookup, no locks
        if let Some(registry) = crate::core::registry::SymbolRegistry::try_global() {
            return registry.lookup(bytes);
        }

        // Fallback: registry not initialized, return None
        // In production, this should not happen after startup
        None
    }

    /// Convert symbol back to string (hot path)
    #[inline]
    pub fn as_str(&self) -> &'static str {
        // Fast path: pre-defined symbols
        match self.0 {
            0 => "BTCUSDT",
            1 => "ETHUSDT",
            2 => "SOLUSDT",
            3 => "BNBUSDT",
            4 => "XRPUSDT",
            5 => "ADAUSDT",
            6 => "DOGEUSDT",
            7 => "AVAXUSDT",
            8 => "TRXUSDT",
            9 => "DOTUSDT",
            10 => "PEPEUSDT",
            _ => {
                // Lookup in registry
                if let Some(registry) = crate::core::registry::SymbolRegistry::try_global() {
                    if let Some(name) = registry.get_name(*self) {
                        return name;
                    }
                }
                "UNKNOWN"
            }
        }
    }

    /// Check if this is a valid symbol (not UNKNOWN)
    #[inline(always)]
    pub const fn is_valid(&self) -> bool {
        self.0 != Self::UNKNOWN.0
    }

    // === Pre-defined common symbols (IDs 0-10) ===
    pub const BTCUSDT: Self = Self(0);
    pub const ETHUSDT: Self = Self(1);
    pub const SOLUSDT: Self = Self(2);
    pub const BNBUSDT: Self = Self(3);
    pub const XRPUSDT: Self = Self(4);
    pub const ADAUSDT: Self = Self(5);
    pub const DOGEUSDT: Self = Self(6);
    pub const AVAXUSDT: Self = Self(7);
    pub const TRXUSDT: Self = Self(8);
    pub const DOTUSDT: Self = Self(9);
    pub const PEPEUSDT: Self = Self(10);
}

impl Default for Symbol {
    #[inline(always)]
    fn default() -> Self {
        Self(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_static_symbols() {
        assert_eq!(Symbol::from_bytes(b"BTCUSDT"), Some(Symbol::BTCUSDT));
        assert_eq!(Symbol::from_bytes(b"ETHUSDT"), Some(Symbol::ETHUSDT));
        assert_eq!(Symbol::from_bytes(b"SOLUSDT"), Some(Symbol::SOLUSDT));
        assert_eq!(Symbol::from_bytes(b"DOTUSDT"), Some(Symbol::DOTUSDT));
    }

    #[test]
    fn test_as_str() {
        assert_eq!(Symbol::BTCUSDT.as_str(), "BTCUSDT");
        assert_eq!(Symbol::ETHUSDT.as_str(), "ETHUSDT");
        assert_eq!(Symbol::DOTUSDT.as_str(), "DOTUSDT");
    }

    #[test]
    fn test_invalid_symbol() {
        assert!(Symbol::from_bytes(b"").is_none());
    }

    #[test]
    fn test_symbol_comparison() {
        let a = Symbol::BTCUSDT;
        let b = Symbol::BTCUSDT;
        let c = Symbol::ETHUSDT;

        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn test_symbol_as_hashmap_key() {
        use std::collections::HashMap;

        let mut map = HashMap::new();
        map.insert(Symbol::BTCUSDT, 100.0);
        map.insert(Symbol::ETHUSDT, 200.0);

        assert_eq!(map.get(&Symbol::BTCUSDT), Some(&100.0));
    }

    #[test]
    fn test_symbol_copy() {
        let a = Symbol::BTCUSDT;
        let b = a;
        let c = a;

        assert_eq!(a, b);
        assert_eq!(a, c);
    }

    #[test]
    fn test_raw_id() {
        assert_eq!(Symbol::BTCUSDT.as_raw(), 0);
        assert_eq!(Symbol::ETHUSDT.as_raw(), 1);
        assert_eq!(Symbol::DOTUSDT.as_raw(), 9);
    }

    #[test]
    fn test_unknown_symbol() {
        assert!(!Symbol::UNKNOWN.is_valid());
        assert!(Symbol::BTCUSDT.is_valid());
    }
}

// Number of pre-defined static symbols (IDs 0-10)
const STATIC_SYMBOL_COUNT: u32 = 11;

// HFT Hot Path Checklist verified:
// ✓ No heap allocations (all stack-based)
// ✓ No panics (all operations return Option)
// ✓ No locks in hot path (registry lookup is read-only)
// ✓ Stack only (Copy type)
// ✓ O(1) lookup via pattern matching + hash
// ✓ No string operations in hot path
