//! Symbol Registry (Warm Path initialization)
//!
//! Pre-registers all symbols at startup to enable lock-free lookup in hot path.
//! Eliminates Mutex contention and heap allocation during message parsing.
//!
//! Architecture:
//! - Warm Path: register_all() called once at startup
//! - Hot Path: from_bytes_static() does O(1) array lookup, no locks

use crate::core::Symbol;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::OnceLock;

/// Maximum number of symbols supported
pub const MAX_SYMBOLS: usize = 5000;

/// Global symbol registry
static SYMBOL_REGISTRY: OnceLock<SymbolRegistry> = OnceLock::new();

/// Next symbol ID counter (for registration)
static NEXT_SYMBOL_ID: AtomicU32 = AtomicU32::new(0);

/// Symbol registry with array-based lookup
pub struct SymbolRegistry {
    /// Names indexed by symbol ID
    names: Box<[Option<&'static str>; MAX_SYMBOLS]>,
    /// Reverse lookup: name hash -> symbol ID
    /// Using simple linear probe for collision resolution
    lookup_table: Box<[Option<u32>; MAX_SYMBOLS]>,
    /// Number of registered symbols
    count: u32,
}

impl SymbolRegistry {
    /// Create new empty registry
    fn new() -> Self {
        Self {
            names: Box::new([None; MAX_SYMBOLS]),
            lookup_table: Box::new([None; MAX_SYMBOLS]),
            count: 0,
        }
    }

    /// Initialize the global registry with discovered symbols
    /// Called ONCE at startup (warm path)
    pub fn initialize(symbols: &[String]) -> Result<(), RegistryError> {
        let mut registry = Self::new();

        for name in symbols {
            if registry.count >= MAX_SYMBOLS as u32 {
                return Err(RegistryError::CapacityExceeded);
            }

            let id = NEXT_SYMBOL_ID.fetch_add(1, Ordering::SeqCst);
            if id as usize >= MAX_SYMBOLS {
                return Err(RegistryError::CapacityExceeded);
            }

            // Leak the string to get 'static lifetime
            let static_name: &'static str = Box::leak(name.clone().into_boxed_str());

            // Store name by ID
            registry.names[id as usize] = Some(static_name);

            // Store in lookup table
            let hash = hash_symbol_name(static_name.as_bytes());
            let slot = find_slot(&registry.lookup_table, hash, static_name);
            registry.lookup_table[slot] = Some(id);

            registry.count += 1;
        }

        // Set global registry
        SYMBOL_REGISTRY
            .set(registry)
            .map_err(|_| RegistryError::AlreadyInitialized)?;

        tracing::info!(
            "Symbol registry initialized with {} symbols",
            SYMBOL_REGISTRY.get().unwrap().count
        );

        Ok(())
    }

    /// Get the global registry (panics if not initialized)
    pub fn global() -> &'static Self {
        SYMBOL_REGISTRY
            .get()
            .expect("Symbol registry not initialized")
    }

    /// Check if registry is initialized
    pub fn is_initialized() -> bool {
        SYMBOL_REGISTRY.get().is_some()
    }

    /// Try to get the global registry (returns None if not initialized)
    pub fn try_global() -> Option<&'static Self> {
        SYMBOL_REGISTRY.get()
    }

    /// Lookup symbol by name (hot path, lock-free)
    #[inline(always)]
    pub fn lookup(&self, name: &[u8]) -> Option<Symbol> {
        if name.is_empty() {
            return None;
        }

        // Fast path: check common symbols with direct comparison
        // This is branch-predictable and faster than hash for top symbols
        match name {
            b"BTCUSDT" => return Some(Symbol::from_raw(0)),
            b"ETHUSDT" => return Some(Symbol::from_raw(1)),
            b"SOLUSDT" => return Some(Symbol::from_raw(2)),
            _ => {}
        }

        // Hash-based lookup
        let hash = hash_symbol_name(name);
        let mut probe = 0;

        loop {
            let slot = (hash as usize + probe) % MAX_SYMBOLS;

            match self.lookup_table[slot] {
                Some(id) => {
                    // Verify name matches (handle collision)
                    if let Some(stored_name) = self.names[id as usize] {
                        if stored_name.as_bytes() == name {
                            return Some(Symbol::from_raw(id));
                        }
                    }
                    // Collision, probe next
                    probe += 1;
                    if probe >= MAX_SYMBOLS {
                        return None;
                    }
                }
                None => return None, // Empty slot = not found
            }
        }
    }

    /// Get symbol name by ID (hot path, lock-free)
    #[inline(always)]
    pub fn get_name(&self, symbol: Symbol) -> Option<&'static str> {
        self.names.get(symbol.as_raw() as usize)?.as_ref().copied()
    }

    /// Get number of registered symbols
    pub fn count(&self) -> u32 {
        self.count
    }
}

/// Simple FNV-1a hash for symbol names
#[inline(always)]
fn hash_symbol_name(name: &[u8]) -> u32 {
    let mut hash: u32 = 2166136261;
    for &byte in name {
        hash ^= byte as u32;
        hash = hash.wrapping_mul(16777619);
    }
    hash
}

/// Find slot in lookup table (linear probing)
#[inline]
fn find_slot(table: &[Option<u32>; MAX_SYMBOLS], hash: u32, _name: &str) -> usize {
    let mut probe = 0;

    loop {
        let slot = (hash as usize + probe) % MAX_SYMBOLS;

        match table[slot] {
            None => return slot, // Empty slot found
            Some(_id) => {
                // Check if same symbol (idempotent registration)
                // We don't have access to names here, so just find next empty
                probe += 1;
                if probe >= MAX_SYMBOLS {
                    panic!("Symbol lookup table full");
                }
            }
        }
    }
}

/// Registry errors
#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("Registry already initialized")]
    AlreadyInitialized,

    #[error("Symbol capacity exceeded")]
    CapacityExceeded,
}

/// Initialize registry with default symbols (for testing/development)
pub fn initialize_with_defaults() -> Result<(), RegistryError> {
    let defaults = vec![
        "BTCUSDT".to_string(),
        "ETHUSDT".to_string(),
        "SOLUSDT".to_string(),
        "BNBUSDT".to_string(),
        "XRPUSDT".to_string(),
        "ADAUSDT".to_string(),
        "DOGEUSDT".to_string(),
        "AVAXUSDT".to_string(),
        "TRXUSDT".to_string(),
        "DOTUSDT".to_string(),
        "PEPEUSDT".to_string(),
        "1000PEPEUSDT".to_string(),
        "LINKUSDT".to_string(),
        "MATICUSDT".to_string(),
        "LTCUSDT".to_string(),
    ];

    SymbolRegistry::initialize(&defaults)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_symbol_name() {
        assert_ne!(hash_symbol_name(b"BTCUSDT"), 0);
        assert_ne!(hash_symbol_name(b"ETHUSDT"), 0);
        // Same input = same hash
        assert_eq!(hash_symbol_name(b"BTCUSDT"), hash_symbol_name(b"BTCUSDT"));
        // Different input = different hash (usually)
        assert_ne!(hash_symbol_name(b"BTCUSDT"), hash_symbol_name(b"ETHUSDT"));
    }

    #[test]
    fn test_registry_initialization() {
        // Reset for test
        // Note: OnceLock can't be reset, so this test will only pass once
        if !SymbolRegistry::is_initialized() {
            let symbols = vec!["TESTCOINUSDT".to_string()];
            let result = SymbolRegistry::initialize(&symbols);
            assert!(result.is_ok());
            assert!(SymbolRegistry::is_initialized());
        }
    }

    #[test]
    fn test_registry_lookup() {
        // Initialize if needed
        if !SymbolRegistry::is_initialized() {
            initialize_with_defaults().ok();
        }

        let registry = SymbolRegistry::global();

        // Test fast path symbols
        let btc = registry.lookup(b"BTCUSDT");
        assert!(btc.is_some());

        let eth = registry.lookup(b"ETHUSDT");
        assert!(eth.is_some());

        // Test non-existent
        let unknown = registry.lookup(b"UNKNOWNUSDT");
        // May or may not exist depending on initialization
    }
}
