//! Exchange-specific implementations

pub mod binance;
pub mod bybit;
pub mod parsing;
pub mod traits;

pub use binance::{BinanceWsClient, BinanceMessage};
pub use bybit::{BybitWsClient, BybitMessage, OrderBookData};
pub use parsing::{BinanceParser, BybitParser};
pub use traits::{AnyExchange, ErrorKind, ExchangeError, ExchangeMessage, WebSocketExchange};

use crate::core::Symbol;
use crate::Result;

/// Enum dispatch for exchange clients
/// Provides static dispatch performance with polymorphic interface
pub enum ExchangeClient {
    Binance(BinanceWsClient),
    Bybit(BybitWsClient),
}

impl ExchangeClient {
    pub async fn connect(&mut self) -> Result<()> {
        match self {
            Self::Binance(c) => c.connect().await,
            Self::Bybit(c) => c.connect(false).await,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Binance(c) => WebSocketExchange::name(c),
            Self::Bybit(c) => WebSocketExchange::name(c),
        }
    }

    pub async fn subscribe_tickers(&mut self, symbols: &[Symbol]) -> Result<()> {
        match self {
            Self::Binance(c) => c.subscribe_tickers(symbols).await,
            Self::Bybit(c) => c.subscribe_tickers(symbols).await,
        }
    }

    pub async fn next_message(&mut self) -> Result<Option<ExchangeMessage>> {
        match self {
            Self::Binance(c) => c.next_message().await,
            Self::Bybit(c) => c.next_message().await,
        }
    }
}

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
