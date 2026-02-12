//! Test utilities for symbol registry initialization
//!
//! All tests should call `init_test_registry()` before using symbols.

use std::sync::Once;

static INIT: Once = Once::new();

/// Initialize registry with all test symbols (called once across all tests)
pub fn init_test_registry() {
    INIT.call_once(|| {
        use crate::core::registry::SymbolRegistry;
        let symbols = vec![
            "BTCUSDT".to_string(),
            "ETHUSDT".to_string(),
            "SOLUSDT".to_string(),
            "DOTUSDT".to_string(),
            "PEPEUSDT".to_string(),
            "BNBUSDT".to_string(),
            "XRPUSDT".to_string(),
            "ADAUSDT".to_string(),
            "DOGEUSDT".to_string(),
            "AVAXUSDT".to_string(),
            "TRXUSDT".to_string(),
            "LINKUSDT".to_string(),
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
        ];
        let _ = SymbolRegistry::initialize(&symbols);
    });
}
