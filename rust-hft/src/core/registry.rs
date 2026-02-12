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

        // Fast path for common symbols (pre-defined IDs)
        match name {
            b"BTCUSDT" => return Some(Symbol::from_raw(0)),
            b"ETHUSDT" => return Some(Symbol::from_raw(1)),
            b"SOLUSDT" => return Some(Symbol::from_raw(2)),
            b"BNBUSDT" => return Some(Symbol::from_raw(3)),
            b"XRPUSDT" => return Some(Symbol::from_raw(4)),
            b"ADAUSDT" => return Some(Symbol::from_raw(5)),
            b"DOGEUSDT" => return Some(Symbol::from_raw(6)),
            b"AVAXUSDT" => return Some(Symbol::from_raw(7)),
            b"TRXUSDT" => return Some(Symbol::from_raw(8)),
            b"DOTUSDT" => return Some(Symbol::from_raw(9)),
            b"PEPEUSDT" => return Some(Symbol::from_raw(10)),
            b"TNSRUSDT" => return Some(Symbol::from_raw(11)),
            b"BERAUSDT" => return Some(Symbol::from_raw(12)),
            b"TRIAUSDT" => return Some(Symbol::from_raw(13)),
            b"BLESSUSDT" => return Some(Symbol::from_raw(14)),
            b"DYDXUSDT" => return Some(Symbol::from_raw(15)),
            b"MYXUSDT" => return Some(Symbol::from_raw(16)),
            b"SKRUSDT" => return Some(Symbol::from_raw(17)),
            b"SONICUSDT" => return Some(Symbol::from_raw(18)),
            b"WIFUSDT" => return Some(Symbol::from_raw(19)),
            b"BONKUSDT" => return Some(Symbol::from_raw(20)),
            b"FLOKIUSDT" => return Some(Symbol::from_raw(21)),
            b"LINKUSDT" => return Some(Symbol::from_raw(22)),
            b"UNIUSDT" => return Some(Symbol::from_raw(23)),
            b"AAVEUSDT" => return Some(Symbol::from_raw(24)),
            b"APTVUSDT" => return Some(Symbol::from_raw(25)),
            b"ARBUSDT" => return Some(Symbol::from_raw(26)),
            b"CATUSDT" => return Some(Symbol::from_raw(27)),
            b"ENAUSDT" => return Some(Symbol::from_raw(28)),
            b"GALAUSDT" => return Some(Symbol::from_raw(29)),
            b"GMTUSDT" => return Some(Symbol::from_raw(30)),
            b"INJUSDT" => return Some(Symbol::from_raw(31)),
            b"NEARUSDT" => return Some(Symbol::from_raw(32)),
            b"OPUSDT" => return Some(Symbol::from_raw(33)),
            b"RNDRUSDT" => return Some(Symbol::from_raw(34)),
            b"SANDUSDT" => return Some(Symbol::from_raw(35)),
            b"SEIUSDT" => return Some(Symbol::from_raw(36)),
            b"STRKUSDT" => return Some(Symbol::from_raw(37)),
            b"SUIUSDT" => return Some(Symbol::from_raw(38)),
            b"TONUSDT" => return Some(Symbol::from_raw(39)),
            b"TURBOUSDT" => return Some(Symbol::from_raw(40)),
            b"VIRTUALUSDT" => return Some(Symbol::from_raw(41)),
            b"WLDUSDT" => return Some(Symbol::from_raw(42)),
            b"KAITOUSDT" => return Some(Symbol::from_raw(43)),
            b"LDOUSDT" => return Some(Symbol::from_raw(44)),
            b"LEVERUSDT" => return Some(Symbol::from_raw(45)),
            b"MEUSDT" => return Some(Symbol::from_raw(46)),
            b"PYTHUSDT" => return Some(Symbol::from_raw(47)),
            _ => {}
        }

        // Hash-based lookup for dynamic symbols
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
        "TNSRUSDT".to_string(),
        "BERAUSDT".to_string(),
        "TRIAUSDT".to_string(),
        "BLESSUSDT".to_string(),
        "DYDXUSDT".to_string(),
        "MYXUSDT".to_string(),
        "SKRUSDT".to_string(),
        "SONICUSDT".to_string(),
        "WIFUSDT".to_string(),
        "BONKUSDT".to_string(),
        "FLOKIUSDT".to_string(),
        "UNIUSDT".to_string(),
        "AAVEUSDT".to_string(),
        "ARBUSDT".to_string(),
        "ENAUSDT".to_string(),
        "GALAUSDT".to_string(),
        "GMTUSDT".to_string(),
        "INJUSDT".to_string(),
        "NEARUSDT".to_string(),
        "OPUSDT".to_string(),
        "RNDRUSDT".to_string(),
        "SANDUSDT".to_string(),
        "SEIUSDT".to_string(),
        "STRKUSDT".to_string(),
        "SUIUSDT".to_string(),
        "TONUSDT".to_string(),
        "TURBOUSDT".to_string(),
        "VIRTUALUSDT".to_string(),
        "WLDUSDT".to_string(),
        "KAITOUSDT".to_string(),
        "LDOUSDT".to_string(),
        "LEVERUSDT".to_string(),
        "MEUSDT".to_string(),
        "PYTHUSDT".to_string(),
    ];
    SymbolRegistry::initialize(&defaults)
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
            let symbols = vec!["TESTCOINUSDT".to_string()];
            assert!(SymbolRegistry::initialize(&symbols).is_ok());
        }
    }
    #[test]
    fn test_registry_lookup() {
        if !SymbolRegistry::is_initialized() {
            initialize_with_defaults().ok();
        }
        let registry = SymbolRegistry::global();
        assert!(registry.lookup(b"BTCUSDT").is_some());
        assert!(registry.lookup(b"ETHUSDT").is_some());
    }
}
