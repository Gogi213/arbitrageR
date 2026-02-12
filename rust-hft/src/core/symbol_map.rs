//! Symbol mapping and normalization
use crate::core::Symbol;
use crate::exchanges::Exchange;

#[derive(Debug, Clone, Copy)]
pub struct SymbolInfo {
    pub symbol: Symbol,
    pub binance_name: &'static str,
    pub bybit_name: &'static str,
}

impl SymbolInfo {
    pub const fn new(symbol: Symbol, name: &'static str) -> Self {
        Self {
            symbol,
            binance_name: name,
            bybit_name: name,
        }
    }
    pub const fn new_mapped(
        symbol: Symbol,
        binance_name: &'static str,
        bybit_name: &'static str,
    ) -> Self {
        Self {
            symbol,
            binance_name,
            bybit_name,
        }
    }
}

// Extended symbol map for all predefined symbols
pub static SYMBOL_MAP: [SymbolInfo; 48] = [
    SymbolInfo::new(Symbol::BTCUSDT, "BTCUSDT"),
    SymbolInfo::new(Symbol::ETHUSDT, "ETHUSDT"),
    SymbolInfo::new(Symbol::SOLUSDT, "SOLUSDT"),
    SymbolInfo::new(Symbol::BNBUSDT, "BNBUSDT"),
    SymbolInfo::new(Symbol::XRPUSDT, "XRPUSDT"),
    SymbolInfo::new(Symbol::ADAUSDT, "ADAUSDT"),
    SymbolInfo::new(Symbol::DOGEUSDT, "DOGEUSDT"),
    SymbolInfo::new(Symbol::AVAXUSDT, "AVAXUSDT"),
    SymbolInfo::new(Symbol::TRXUSDT, "TRXUSDT"),
    SymbolInfo::new(Symbol::DOTUSDT, "DOTUSDT"),
    // Edge case: PEPEUSDT maps to 1000PEPEUSDT on some exchanges
    SymbolInfo::new_mapped(Symbol::PEPEUSDT, "1000PEPEUSDT", "1000PEPEUSDT"),
    // Additional symbols (IDs 11-47)
    SymbolInfo::new(Symbol::TNSRUSDT, "TNSRUSDT"),
    SymbolInfo::new(Symbol::BERAUSDT, "BERAUSDT"),
    SymbolInfo::new(Symbol::TRIAUSDT, "TRIAUSDT"),
    SymbolInfo::new(Symbol::BLESSUSDT, "BLESSUSDT"),
    SymbolInfo::new(Symbol::DYDXUSDT, "DYDXUSDT"),
    SymbolInfo::new(Symbol::MYXUSDT, "MYXUSDT"),
    SymbolInfo::new(Symbol::SKRUSDT, "SKRUSDT"),
    SymbolInfo::new(Symbol::SONICUSDT, "SONICUSDT"),
    SymbolInfo::new(Symbol::WIFUSDT, "WIFUSDT"),
    SymbolInfo::new(Symbol::BONKUSDT, "BONKUSDT"),
    SymbolInfo::new(Symbol::FLOKIUSDT, "FLOKIUSDT"),
    SymbolInfo::new(Symbol::LINKUSDT, "LINKUSDT"),
    SymbolInfo::new(Symbol::UNIUSDT, "UNIUSDT"),
    SymbolInfo::new(Symbol::AAVEUSDT, "AAVEUSDT"),
    SymbolInfo::new(Symbol::APTVUSDT, "APTVUSDT"),
    SymbolInfo::new(Symbol::ARBUSDT, "ARBUSDT"),
    SymbolInfo::new(Symbol::CATUSDT, "CATUSDT"),
    SymbolInfo::new(Symbol::ENAUSDT, "ENAUSDT"),
    SymbolInfo::new(Symbol::GALAUSDT, "GALAUSDT"),
    SymbolInfo::new(Symbol::GMTUSDT, "GMTUSDT"),
    SymbolInfo::new(Symbol::INJUSDT, "INJUSDT"),
    SymbolInfo::new(Symbol::NEARUSDT, "NEARUSDT"),
    SymbolInfo::new(Symbol::OPUSDT, "OPUSDT"),
    SymbolInfo::new(Symbol::RNDRUSDT, "RNDRUSDT"),
    SymbolInfo::new(Symbol::SANDUSDT, "SANDUSDT"),
    SymbolInfo::new(Symbol::SEIUSDT, "SEIUSDT"),
    SymbolInfo::new(Symbol::STRKUSDT, "STRKUSDT"),
    SymbolInfo::new(Symbol::SUIUSDT, "SUIUSDT"),
    SymbolInfo::new(Symbol::TONUSDT, "TONUSDT"),
    SymbolInfo::new(Symbol::TURBOUSDT, "TURBOUSDT"),
    SymbolInfo::new(Symbol::VIRTUALUSDT, "VIRTUALUSDT"),
    SymbolInfo::new(Symbol::WLDUSDT, "WLDUSDT"),
    SymbolInfo::new(Symbol::KAITOUSDT, "KAITOUSDT"),
    SymbolInfo::new(Symbol::LDOUSDT, "LDOUSDT"),
    SymbolInfo::new(Symbol::LEVERUSDT, "LEVERUSDT"),
    SymbolInfo::new(Symbol::MEUSDT, "MEUSDT"),
    SymbolInfo::new(Symbol::PYTHUSDT, "PYTHUSDT"),
];

pub struct SymbolMapper;

impl SymbolMapper {
    #[inline]
    pub fn get_name(symbol: Symbol, exchange: Exchange) -> Option<&'static str> {
        for info in SYMBOL_MAP.iter() {
            if info.symbol == symbol {
                return Some(match exchange {
                    Exchange::Binance => info.binance_name,
                    Exchange::Bybit => info.bybit_name,
                });
            }
        }
        // Fallback to Symbol's internal string (for dynamic symbols)
        Some(symbol.as_str())
    }

    #[inline]
    pub fn from_exchange_name(name: &str, exchange: Exchange) -> Option<Symbol> {
        // Linear scan for mapped names first
        for info in SYMBOL_MAP.iter() {
            let match_found = match exchange {
                Exchange::Binance => info.binance_name == name,
                Exchange::Bybit => info.bybit_name == name,
            };
            if match_found {
                return Some(info.symbol);
            }
        }

        // Fallback: Try standard parsing
        Symbol::from_bytes(name.as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_get_name_binance() {
        assert_eq!(
            SymbolMapper::get_name(Symbol::BTCUSDT, Exchange::Binance),
            Some("BTCUSDT")
        );
    }
    #[test]
    fn test_get_name_bybit() {
        assert_eq!(
            SymbolMapper::get_name(Symbol::ETHUSDT, Exchange::Bybit),
            Some("ETHUSDT")
        );
    }
    #[test]
    fn test_from_exchange_name() {
        assert_eq!(
            SymbolMapper::from_exchange_name("BTCUSDT", Exchange::Binance),
            Some(Symbol::BTCUSDT)
        );
    }
    #[test]
    fn test_edge_case_pepe() {
        assert_eq!(
            SymbolMapper::get_name(Symbol::PEPEUSDT, Exchange::Binance),
            Some("1000PEPEUSDT")
        );
        assert_eq!(
            SymbolMapper::from_exchange_name("1000PEPEUSDT", Exchange::Binance),
            Some(Symbol::PEPEUSDT)
        );
        assert_eq!(
            SymbolMapper::from_exchange_name("1000PEPEUSDT", Exchange::Bybit),
            Some(Symbol::PEPEUSDT)
        );
    }
}
