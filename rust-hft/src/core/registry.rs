//! Symbol interning for zero-allocation string handling
//!
//! Symbols are stored as u32 IDs with pre-registered lookup.
//! Zero-allocation parsing from JSON byte slices.

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
    names: Box<[Option<&'static str>; MAX_SYMBOLS]>,
    lookup_table: Box<[Option<u32>; MAX_SYMBOLS]>,
    count: u32,
}

impl SymbolRegistry {
    fn new() -> Self {
        Self {
            names: Box::new([None; MAX_SYMBOLS]),
            lookup_table: Box::new([None; MAX_SYMBOLS]),
            count: 0,
        }
    }

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

            let static_name: &'static str = Box::leak(name.clone().into_boxed_str());
            registry.names[id as usize] = Some(static_name);

            let hash = hash_symbol_name(static_name.as_bytes());
            let slot = find_slot(&registry.lookup_table, hash, static_name);
            registry.lookup_table[slot] = Some(id);
            registry.count += 1;
        }

        SYMBOL_REGISTRY
            .set(registry)
            .map_err(|_| RegistryError::AlreadyInitialized)?;
        tracing::info!(
            "Symbol registry initialized with {} symbols",
            SYMBOL_REGISTRY.get().unwrap().count
        );
        Ok(())
    }

    pub fn try_global() -> Option<&'static Self> {
        SYMBOL_REGISTRY.get()
    }

    pub fn lookup(&self, name: &[u8]) -> Option<Symbol> {
        if name.is_empty() {
            return None;
        }

        // Hash-based lookup for all symbols
        let hash = hash_symbol_name(name);
        let mut probe = 0;
        loop {
            let slot = (hash as usize + probe) % MAX_SYMBOLS;
            match self.lookup_table[slot] {
                Some(id) => {
                    if let Some(stored_name) = self.names[id as usize] {
                        if stored_name.as_bytes() == name {
                            return Some(Symbol::from_raw(id));
                        }
                    }
                    probe += 1;
                    if probe >= MAX_SYMBOLS {
                        return None;
                    }
                }
                None => return None,
            }
        }
    }

    pub fn get_name(&self, symbol: Symbol) -> Option<&'static str> {
        self.names.get(symbol.as_raw() as usize)?.as_ref().copied()
    }

    pub fn count(&self) -> u32 {
        self.count
    }

    pub fn is_initialized() -> bool {
        SYMBOL_REGISTRY.get().is_some()
    }
}

#[inline(always)]
fn hash_symbol_name(name: &[u8]) -> u32 {
    let mut hash: u32 = 2166136261;
    for &byte in name {
        hash ^= byte as u32;
        hash = hash.wrapping_mul(16777619);
    }
    hash
}

#[inline]
fn find_slot(table: &[Option<u32>; MAX_SYMBOLS], hash: u32, _name: &str) -> usize {
    let mut probe = 0;
    loop {
        let slot = (hash as usize + probe) % MAX_SYMBOLS;
        match table[slot] {
            None => return slot,
            Some(_id) => {
                probe += 1;
                if probe >= MAX_SYMBOLS {
                    panic!("Symbol lookup table full");
                }
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    #[error("Registry already initialized")]
    AlreadyInitialized,
    #[error("Symbol capacity exceeded")]
    CapacityExceeded,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_hash_symbol_name() {
        assert_ne!(hash_symbol_name(b"BTCUSDT"), 0);
        assert_eq!(hash_symbol_name(b"BTCUSDT"), hash_symbol_name(b"BTCUSDT"));
        assert_ne!(hash_symbol_name(b"BTCUSDT"), hash_symbol_name(b"ETHUSDT"));
    }
    #[test]
    fn test_registry_initialization() {
        if !SymbolRegistry::is_initialized() {
            let symbols = vec!["BTCUSDT".to_string(), "ETHUSDT".to_string()];
            assert!(SymbolRegistry::initialize(&symbols).is_ok());
        }
    }
    #[test]
    fn test_registry_lookup() {
        if !SymbolRegistry::is_initialized() {
            let symbols = vec!["BTCUSDT".to_string(), "ETHUSDT".to_string()];
            SymbolRegistry::initialize(&symbols).ok();
        }
        let registry = SymbolRegistry::try_global().unwrap();
        assert!(registry.lookup(b"BTCUSDT").is_some());
        assert!(registry.lookup(b"ETHUSDT").is_some());
    }
}
