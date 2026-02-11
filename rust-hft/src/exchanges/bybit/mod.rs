//! Bybit Futures WebSocket client (V5 API)
//!
//! Native WebSocket client for Bybit Futures exchange using V5 API.
//! Handles public trade and ticker streams.

use crate::core::{FixedPoint8, Side, Symbol, TickerData, TradeData};
use crate::ws::connection::{WebSocketConnection, WebSocketError};
use crate::ws::subscription::{StreamType, SubscriptionManager};
use crate::ws::ping::{PingHandler, ConnectionMonitor};
use crate::exchanges::parsing::{BybitParser, BybitMessageType};
use crate::{HftError, Result};
use std::time::Duration;
use tokio::time::{interval, Instant};

/// Bybit Futures WebSocket client (V5 API)
pub struct BybitWsClient {
    /// WebSocket connection
    connection: Option<WebSocketConnection>,
    /// Subscription manager
    subscriptions: SubscriptionManager,
    /// Connection monitor (ping/pong)
    monitor: ConnectionMonitor,
    /// Last message timestamp
    last_message: Instant,
    /// Request ID counter for V5 protocol
    request_id: u32,
}

impl BybitWsClient {
    /// Bybit Futures WebSocket URL (Linear/Perpetual contracts)
    pub const WS_URL: &'static str = "wss://stream.bybit.com/v5/public/linear";
    /// Bybit Testnet URL
    pub const WS_URL_TESTNET: &'static str = "wss://stream-testnet.bybit.com/v5/public/linear";
    
    /// Create new Bybit client
    pub fn new() -> Self {
        Self {
            connection: None,
            subscriptions: SubscriptionManager::new(),
            monitor: ConnectionMonitor::new("bybit".to_string()),
            last_message: Instant::now(),
            request_id: 0,
        }
    }

    /// Create new Bybit client for testnet
    pub fn new_testnet() -> Self {
        let mut client = Self::new();
        client.monitor = ConnectionMonitor::new("bybit-testnet".to_string());
        client
    }

    /// Connect to Bybit WebSocket
    pub async fn connect(&mut self, testnet: bool) -> Result<()> {
        let url = if testnet { Self::WS_URL_TESTNET } else { Self::WS_URL };
        
        let mut conn = WebSocketConnection::connect(url)
            .await
            .map_err(|e| HftError::WebSocket(e.to_string()))?;
        
        self.monitor = ConnectionMonitor::new(
            if testnet { "bybit-testnet".to_string() } else { "bybit".to_string() }
        );
        self.connection = Some(conn);
        
        Ok(())
    }

    /// Subscribe to public trade stream for symbols
    /// 
    /// Bybit V5 uses topics: publicTrade.{symbol}
    pub async fn subscribe_public_trades(&mut self, symbols: &[Symbol]) -> Result<()> {
        if symbols.is_empty() {
            return Ok(());
        }

        // Request subscription
        self.subscriptions.request_subscription(symbols, StreamType::Trade);
        
        // Create topics for V5 protocol
        let topics: Vec<String> = symbols
            .iter()
            .map(|s| format!("publicTrade.{}", s.as_str()))
            .collect();
        
        // Send V5 subscription message
        let args: Vec<serde_json::Value> = topics
            .iter()
            .map(|t| serde_json::json!({"topic": t}))
            .collect();
        
        let args: Vec<serde_json::Value> = topics
            .iter()
            .map(|t| serde_json::json!({"topic": t}))
            .collect();
        
        let subscribe_msg = serde_json::json!({
            "op": "subscribe",
            "args": args,
        });
        
        if let Some(conn) = self.connection.as_mut() {
            conn.send_text(&subscribe_msg.to_string())
                .await
                .map_err(|e| HftError::WebSocket(e.to_string()))?;
        }
        
        Ok(())
    }

    /// Subscribe to ticker stream for symbols
    /// 
    /// Bybit V5 uses topics: tickers.{symbol}
    pub async fn subscribe_tickers(&mut self, symbols: &[Symbol]) -> Result<()> {
        if symbols.is_empty() {
            return Ok(());
        }

        self.subscriptions.request_subscription(symbols, StreamType::Ticker);
        
        let topics: Vec<String> = symbols
            .iter()
            .map(|s| format!("tickers.{}", s.as_str()))
            .collect();
        
        let args: Vec<serde_json::Value> = topics
            .iter()
            .map(|t| serde_json::json!({"topic": t}))
            .collect();
        
        let subscribe_msg = serde_json::json!({
            "op": "subscribe",
            "args": args,
        });
        
        if let Some(conn) = self.connection.as_mut() {
            conn.send_text(&subscribe_msg.to_string())
                .await
                .map_err(|e| HftError::WebSocket(e.to_string()))?;
        }
        
        Ok(())
    }

    /// Subscribe to orderbook stream for symbols
    /// 
    /// Bybit V5 uses topics: orderbook.1.{symbol} (level 1)
    pub async fn subscribe_orderbook(&mut self, symbols: &[Symbol]) -> Result<()> {
        if symbols.is_empty() {
            return Ok(());
        }

        self.subscriptions.request_subscription(symbols, StreamType::OrderBook);
        
        let topics: Vec<String> = symbols
            .iter()
            .map(|s| format!("orderbook.1.{}", s.as_str()))
            .collect();
        
        let args: Vec<serde_json::Value> = topics
            .iter()
            .map(|t| serde_json::json!({"topic": t}))
            .collect();
        
        let subscribe_msg = serde_json::json!({
            "op": "subscribe",
            "args": args,
        });
        
        if let Some(conn) = self.connection.as_mut() {
            conn.send_text(&subscribe_msg.to_string())
                .await
                .map_err(|e| HftError::WebSocket(e.to_string()))?;
        }
        
        Ok(())
    }

    /// Receive and process next message
    pub async fn recv(&mut self) -> Result<Option<BybitMessage>> {
        if let Some(conn) = self.connection.as_mut() {
            match conn.recv().await {
                Ok(Some(msg)) => {
                    self.last_message = Instant::now();
                    self.monitor.record_activity();
                    
                    // Parse message
                    if let Some(text) = msg.to_text().ok() {
                        return self.parse_message(text);
                    }
                }
                Ok(None) => {
                    // Connection closed
                    self.connection = None;
                    return Ok(None);
                }
                Err(e) => {
                    return Err(HftError::WebSocket(e.to_string()));
                }
            }
        }
        
        Ok(None)
    }

    /// Parse Bybit V5 message
    fn parse_message(&mut self, text: &str) -> Result<Option<BybitMessage>> {
        let data = text.as_bytes();

        // Detect message type and parse accordingly
        match BybitParser::detect_message_type(data) {
            BybitMessageType::PublicTrade => {
                match BybitParser::parse_public_trade(data) {
                    Some(result) => Ok(Some(BybitMessage::Trade(result.data))),
                    None => Ok(None),
                }
            }
            BybitMessageType::Ticker => {
                match BybitParser::parse_ticker(data) {
                    Some(result) => Ok(Some(BybitMessage::Ticker(result.data))),
                    None => Ok(None),
                }
            }
            BybitMessageType::Pong => {
                Ok(Some(BybitMessage::Pong))
            }
            BybitMessageType::SubscriptionResponse => {
                Ok(Some(BybitMessage::SubscriptionSuccess))
            }
            BybitMessageType::Unknown => {
                // Unknown message type
                Ok(None)
            }
        }
    }

    /// Check if connected
    pub fn is_connected(&self) -> bool {
        self.connection.as_ref()
            .map(|c| c.is_connected())
            .unwrap_or(false)
    }

    /// Get connection health
    pub fn health(&self) -> bool {
        self.monitor.is_healthy()
    }

    /// Get last message time
    pub fn last_message_time(&self) -> Instant {
        self.last_message
    }

    /// Send ping (Bybit requires explicit ping)
    pub async fn send_ping(&mut self) -> Result<()> {
        if let Some(conn) = self.connection.as_mut() {
            let ping_msg = serde_json::json!({
                "op": "ping",
            });
            conn.send_text(&ping_msg.to_string())
                .await
                .map_err(|e| HftError::WebSocket(e.to_string()))?;
        }
        Ok(())
    }

    /// Get active trade subscriptions
    pub fn active_trade_subscriptions(&self) -> Vec<Symbol> {
        self.subscriptions.get_active(StreamType::Trade)
    }

    /// Get active ticker subscriptions
    pub fn active_ticker_subscriptions(&self) -> Vec<Symbol> {
        self.subscriptions.get_active(StreamType::Ticker)
    }
}

impl Default for BybitWsClient {
    fn default() -> Self {
        Self::new()
    }
}

// === WebSocketExchange Trait Implementation ===

use crate::exchanges::traits::{ErrorKind, ExchangeError, ExchangeMessage, WebSocketExchange};
use crate::exchanges::Exchange;

impl WebSocketExchange for BybitWsClient {
    #[inline]
    fn exchange(&self) -> Exchange {
        Exchange::Bybit
    }

    async fn connect(&mut self) -> crate::Result<()> {
        // Use existing connect method (default to mainnet)
        // If testnet is needed, it should be configured at creation time
        self.connect(false).await
    }

    async fn subscribe_trades(&mut self, symbols: &[Symbol]) -> crate::Result<()> {
        self.subscribe_public_trades(symbols).await
    }

    async fn subscribe_tickers(&mut self, symbols: &[Symbol]) -> crate::Result<()> {
        self.subscribe_tickers(symbols).await
    }

    async fn next_message(&mut self) -> crate::Result<Option<ExchangeMessage>> {
        match self.recv().await? {
            Some(BybitMessage::Trade(trade)) => {
                Ok(Some(ExchangeMessage::Trade(Exchange::Bybit, trade)))
            }
            Some(BybitMessage::Ticker(ticker)) => {
                Ok(Some(ExchangeMessage::Ticker(Exchange::Bybit, ticker)))
            }
            Some(BybitMessage::Pong) | Some(BybitMessage::SubscriptionSuccess) => {
                Ok(Some(ExchangeMessage::Heartbeat))
            }
            Some(BybitMessage::OrderBook(_)) => {
                // Not yet supported in generic ExchangeMessage
                Ok(None)
            }
            Some(BybitMessage::Error(msg)) => {
                Ok(Some(ExchangeMessage::Error(ExchangeError {
                    exchange: Exchange::Bybit,
                    kind: ErrorKind::Unknown,
                    message: msg,
                })))
            }
            None => Ok(None),
        }
    }

    #[inline]
    fn is_connected(&self) -> bool {
        self.connection.as_ref()
            .map(|c| c.is_connected())
            .unwrap_or(false)
    }

    #[inline]
    fn last_activity(&self) -> std::time::Instant {
        self.last_message.into_std()
    }
}

/// Bybit message types
#[derive(Debug, Clone)]
pub enum BybitMessage {
    /// Public trade data
    Trade(TradeData),
    /// Ticker data
    Ticker(TickerData),
    /// Orderbook data
    OrderBook(OrderBookData),
    /// Subscription success response
    SubscriptionSuccess,
    /// Pong response
    Pong,
    /// Error message
    Error(String),
}

/// Order book data structure
#[derive(Debug, Clone)]
pub struct OrderBookData {
    pub symbol: Symbol,
    pub bids: Vec<(FixedPoint8, FixedPoint8)>, // (price, qty)
    pub asks: Vec<(FixedPoint8, FixedPoint8)>,
    pub timestamp: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bybit_client_creation() {
        let client = BybitWsClient::new();
        assert!(!client.is_connected());
        assert_eq!(client.request_id, 0);
    }

    #[test]
    fn test_bybit_client_testnet() {
        let client = BybitWsClient::new_testnet();
        assert!(!client.is_connected());
    }

    #[test]
    fn test_bybit_urls() {
        assert_eq!(BybitWsClient::WS_URL, "wss://stream.bybit.com/v5/public/linear");
        assert_eq!(BybitWsClient::WS_URL_TESTNET, "wss://stream-testnet.bybit.com/v5/public/linear");
    }
}

// TODO Phase 3.3: Implement zero-copy parsing for Bybit V5 format
// Bybit V5 format example:
// {
//   "topic": "publicTrade.BTCUSDT",
//   "type": "snapshot",
//   "ts": 1672304484973,
//   "data": [
//     {
//       "T": 1672304484972,
//       "s": "BTCUSDT",
//       "S": "Buy",
//       "v": "0.001",
//       "p": "16500.50"
//     }
//   ]
// }
// Benchmark target: <5μs for JSON → TradeData/TickerData
