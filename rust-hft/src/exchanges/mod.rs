//! Exchange-specific implementations

pub mod binance;
pub mod bybit;

pub use binance::BinanceWsClient;
pub use bybit::BybitWsClient;

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
