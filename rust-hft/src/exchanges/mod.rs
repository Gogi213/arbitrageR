//! Exchange-specific implementations

pub mod binance;
pub mod bybit;
pub mod parsing;

pub use binance::{BinanceWsClient, BinanceMessage};
pub use bybit::{BybitWsClient, BybitMessage, OrderBookData};
pub use parsing::{BinanceParser, BybitParser};

/// Exchange identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Exchange {
    Binance,
    Bybit,
}

impl Exchange {
    pub fn name(&self) -> &'static str {
        match self {
            Exchange::Binance => "binance",
            Exchange::Bybit => "bybit",
        }
    }
}
