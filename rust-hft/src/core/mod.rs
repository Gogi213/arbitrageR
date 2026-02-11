//! Core types for zero-allocation HFT operations
//! 
//! This module contains the fundamental types used throughout the system:
//! - FixedPoint8: Fixed-point arithmetic for prices
//! - Symbol: Interned string for trading pairs
//! - TickerData: Best bid/ask data
//! - TradeData: Individual trade information

pub mod fixed_point;
pub mod symbol;
pub mod symbol_map;
pub mod market_data;

pub use fixed_point::FixedPoint8;
pub use market_data::{Side, TickerData, TradeData};
pub use symbol::Symbol;
pub use symbol_map::{SymbolInfo, SymbolMapper};
