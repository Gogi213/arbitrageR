//! Core types for zero-allocation HFT operations
//! 
//! This module contains the fundamental types used throughout the system:
//! - FixedPoint8: Fixed-point arithmetic for prices
//! - Symbol: Interned string for trading pairs
//! - TickerData: Best bid/ask data
//! - TradeData: Individual trade information
//! - SymbolDiscovery: Dynamic symbol loading (cold path)
//! - SymbolRegistry: Pre-registration for hot path lookups

pub mod discovery;
pub mod fixed_point;
pub mod market_data;
pub mod registry;
pub mod symbol;
pub mod symbol_map;

pub use discovery::{DiscoveredSymbol, DiscoveryError, SymbolDiscovery, DEFAULT_MIN_VOLUME};
pub use fixed_point::FixedPoint8;
pub use market_data::{Side, TickerData, TradeData};
pub use registry::{SymbolRegistry, RegistryError, MAX_SYMBOLS};
pub use symbol::Symbol;
pub use symbol_map::SymbolMapper;
