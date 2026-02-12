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

        // Fast path: check pre-defined symbols with direct comparison
        match bytes {
            b"BTCUSDT" => return Some(Self::BTCUSDT),
            b"ETHUSDT" => return Some(Self::ETHUSDT),
            b"SOLUSDT" => return Some(Self::SOLUSDT),
            b"BNBUSDT" => return Some(Self::BNBUSDT),
            b"XRPUSDT" => return Some(Self::XRPUSDT),
            b"ADAUSDT" => return Some(Self::ADAUSDT),
            b"DOGEUSDT" => return Some(Self::DOGEUSDT),
            b"AVAXUSDT" => return Some(Self::AVAXUSDT),
            b"TRXUSDT" => return Some(Self::TRXUSDT),
            b"DOTUSDT" => return Some(Self::DOTUSDT),
            b"PEPEUSDT" => return Some(Self::PEPEUSDT),
            b"TNSRUSDT" => return Some(Self::TNSRUSDT),
            b"BERAUSDT" => return Some(Self::BERAUSDT),
            b"TRIAUSDT" => return Some(Self::TRIAUSDT),
            b"BLESSUSDT" => return Some(Self::BLESSUSDT),
            b"DYDXUSDT" => return Some(Self::DYDXUSDT),
            b"MYXUSDT" => return Some(Self::MYXUSDT),
            b"SKRUSDT" => return Some(Self::SKRUSDT),
            b"SONICUSDT" => return Some(Self::SONICUSDT),
            b"WIFUSDT" => return Some(Self::WIFUSDT),
            b"BONKUSDT" => return Some(Self::BONKUSDT),
            b"FLOKIUSDT" => return Some(Self::FLOKIUSDT),
            b"LINKUSDT" => return Some(Self::LINKUSDT),
            b"UNIUSDT" => return Some(Self::UNIUSDT),
            b"AAVEUSDT" => return Some(Self::AAVEUSDT),
            b"APTVUSDT" => return Some(Self::APTVUSDT),
            b"ARBUSDT" => return Some(Self::ARBUSDT),
            b"CATUSDT" => return Some(Self::CATUSDT),
            b"ENAUSDT" => return Some(Self::ENAUSDT),
            b"GALAUSDT" => return Some(Self::GALAUSDT),
            b"GMTUSDT" => return Some(Self::GMTUSDT),
            b"INJUSDT" => return Some(Self::INJUSDT),
            b"NEARUSDT" => return Some(Self::NEARUSDT),
            b"OPUSDT" => return Some(Self::OPUSDT),
            b"RNDRUSDT" => return Some(Self::RNDRUSDT),
            b"SANDUSDT" => return Some(Self::SANDUSDT),
            b"SEIUSDT" => return Some(Self::SEIUSDT),
            b"STRKUSDT" => return Some(Self::STRKUSDT),
            b"SUIUSDT" => return Some(Self::SUIUSDT),
            b"TONUSDT" => return Some(Self::TONUSDT),
            b"TURBOUSDT" => return Some(Self::TURBOUSDT),
            b"VIRTUALUSDT" => return Some(Self::VIRTUALUSDT),
            b"WLDUSDT" => return Some(Self::WLDUSDT),
            b"KAITOUSDT" => return Some(Self::KAITOUSDT),
            b"LDOUSDT" => return Some(Self::LDOUSDT),
            b"LEVERUSDT" => return Some(Self::LEVERUSDT),
            b"MEUSDT" => return Some(Self::MEUSDT),
            b"PYTHUSDT" => return Some(Self::PYTHUSDT),
            _ => {}
        }

        // Lookup in registry (if initialized)
        if let Some(registry) = crate::core::registry::SymbolRegistry::try_global() {
            return registry.lookup(bytes);
        }

        None
    }

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
            11 => "TNSRUSDT",
            12 => "BERAUSDT",
            13 => "TRIAUSDT",
            14 => "BLESSUSDT",
            15 => "DYDXUSDT",
            16 => "MYXUSDT",
            17 => "SKRUSDT",
            18 => "SONICUSDT",
            19 => "WIFUSDT",
            20 => "BONKUSDT",
            21 => "FLOKIUSDT",
            22 => "LINKUSDT",
            23 => "UNIUSDT",
            24 => "AAVEUSDT",
            25 => "APTVUSDT",
            26 => "ARBUSDT",
            27 => "CATUSDT",
            28 => "ENAUSDT",
            29 => "GALAUSDT",
            30 => "GMTUSDT",
            31 => "INJUSDT",
            32 => "NEARUSDT",
            33 => "OPUSDT",
            34 => "RNDRUSDT",
            35 => "SANDUSDT",
            36 => "SEIUSDT",
            37 => "STRKUSDT",
            38 => "SUIUSDT",
            39 => "TONUSDT",
            40 => "TURBOUSDT",
            41 => "VIRTUALUSDT",
            42 => "WLDUSDT",
            43 => "KAITOUSDT",
            44 => "LDOUSDT",
            45 => "LEVERUSDT",
            46 => "MEUSDT",
            47 => "PYTHUSDT",
            _ => {
                if let Some(registry) = crate::core::registry::SymbolRegistry::try_global() {
                    if let Some(name) = registry.get_name(*self) {
                        return name;
                    }
                }
                "UNKNOWN"
            }
        }
    }

    #[inline(always)]
    pub const fn is_valid(&self) -> bool {
        self.0 != Self::UNKNOWN.0
    }

    // Pre-defined common symbols (IDs 0-47)
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
    pub const TNSRUSDT: Self = Self(11);
    pub const BERAUSDT: Self = Self(12);
    pub const TRIAUSDT: Self = Self(13);
    pub const BLESSUSDT: Self = Self(14);
    pub const DYDXUSDT: Self = Self(15);
    pub const MYXUSDT: Self = Self(16);
    pub const SKRUSDT: Self = Self(17);
    pub const SONICUSDT: Self = Self(18);
    pub const WIFUSDT: Self = Self(19);
    pub const BONKUSDT: Self = Self(20);
    pub const FLOKIUSDT: Self = Self(21);
    pub const LINKUSDT: Self = Self(22);
    pub const UNIUSDT: Self = Self(23);
    pub const AAVEUSDT: Self = Self(24);
    pub const APTVUSDT: Self = Self(25);
    pub const ARBUSDT: Self = Self(26);
    pub const CATUSDT: Self = Self(27);
    pub const ENAUSDT: Self = Self(28);
    pub const GALAUSDT: Self = Self(29);
    pub const GMTUSDT: Self = Self(30);
    pub const INJUSDT: Self = Self(31);
    pub const NEARUSDT: Self = Self(32);
    pub const OPUSDT: Self = Self(33);
    pub const RNDRUSDT: Self = Self(34);
    pub const SANDUSDT: Self = Self(35);
    pub const SEIUSDT: Self = Self(36);
    pub const STRKUSDT: Self = Self(37);
    pub const SUIUSDT: Self = Self(38);
    pub const TONUSDT: Self = Self(39);
    pub const TURBOUSDT: Self = Self(40);
    pub const VIRTUALUSDT: Self = Self(41);
    pub const WLDUSDT: Self = Self(42);
    pub const KAITOUSDT: Self = Self(43);
    pub const LDOUSDT: Self = Self(44);
    pub const LEVERUSDT: Self = Self(45);
    pub const MEUSDT: Self = Self(46);
    pub const PYTHUSDT: Self = Self(47);
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
