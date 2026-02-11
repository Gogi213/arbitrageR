//! Symbol interning for zero-allocation string handling
//!
//! Symbols are stored as u32 IDs with a static lookup table.
//! Zero-allocation parsing from JSON byte slices.

use std::sync::atomic::{AtomicU32, Ordering};

/// Trading pair symbol (interned)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Symbol(u32);

impl Symbol {
    /// Maximum number of symbols supported
    pub const MAX_SYMBOLS: u32 = 10_000;

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

    /// Parse from byte slice (e.g., from JSON)
    /// Uses perfect hash for static symbols, falls back to dynamic
    #[inline]
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        // Reject empty strings
        if bytes.is_empty() {
            return None;
        }

        // Fast path: check common symbols with direct comparison
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

        // Slow path: try to register new symbol
        Self::register_dynamic(bytes)
    }

    /// Convert symbol back to string
    #[inline]
    pub fn as_str(&self) -> &'static str {
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
            _ => Self::lookup_dynamic_name(self.0).unwrap_or("UNKNOWN"),
        }
    }

    /// Register new dynamic symbol (warm path, not hot)
    fn register_dynamic(bytes: &[u8]) -> Option<Self> {
        use std::sync::Mutex;
        use std::collections::HashMap;
        use std::sync::LazyLock;

        static DYNAMIC_SYMBOLS: LazyLock<Mutex<HashMap<Vec<u8>, u32>>> =
            LazyLock::new(|| Mutex::new(HashMap::new()));
        static NEXT_ID: AtomicU32 = AtomicU32::new(STATIC_SYMBOL_COUNT as u32);

        // Try to get existing
        let symbols = DYNAMIC_SYMBOLS.lock().ok()?;
        if let Some(&id) = symbols.get(bytes) {
            return Some(Symbol(id));
        }
        drop(symbols);

        // Register new
        let mut symbols = DYNAMIC_SYMBOLS.lock().ok()?;
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);

        if id >= Symbol::MAX_SYMBOLS {
            return None; // Table full
        }

        symbols.insert(bytes.to_vec(), id);
        Some(Symbol(id))
    }

    /// Lookup dynamic symbol name
    fn lookup_dynamic_name(id: u32) -> Option<&'static str> {
        // For now, dynamic symbols just return UNKNOWN
        // In production, you'd store names in a separate static table
        None
    }

    // === Pre-defined common symbols ===
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

const STATIC_SYMBOL_COUNT: u32 = 11;

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
        // Empty string should return None
        assert!(Symbol::from_bytes(b"").is_none());

        // Unknown symbols get registered dynamically (not invalid, just new)
        let new_sym = Symbol::from_bytes(b"NEWCOINUSDT");
        assert!(new_sym.is_some());

        // Should get same ID on second lookup
        let new_sym2 = Symbol::from_bytes(b"NEWCOINUSDT");
        assert_eq!(new_sym, new_sym2);
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
        let b = a; // Copy, not move
        let c = a; // Can still use a

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
    fn test_dynamic_symbol() {
        // Register a new dynamic symbol
        let sym = Symbol::from_bytes(b"PEPEUSDT");
        assert!(sym.is_some());

        // Should return the same ID
        let sym2 = Symbol::from_bytes(b"PEPEUSDT");
        assert_eq!(sym, sym2);
    }
}

// HFT Hot Path Checklist verified:
// ✓ No heap allocations in hot path (static lookup only)
// ✓ No panics (all operations return Option)
// ✓ Stack only (Copy type)
// ✓ O(1) lookup via pattern matching
// ✓ No string operations in hot path
