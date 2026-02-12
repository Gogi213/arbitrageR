//! Bybit Futures WebSocket client (V5 API)
//!
//! Native WebSocket client for Bybit Futures exchange using V5 API.
//! Handles public trade and ticker streams.

use crate::core::{FixedPoint8, Side, Symbol, TickerData, TradeData, SymbolMapper};
use crate::ws::connection::{WebSocketConnection, WebSocketError};
use crate::ws::subscription::{StreamType, SubscriptionManager};
use crate::ws::ping::{PingHandler, ConnectionMonitor};
use crate::exchanges::parsing::{BybitParser, BybitMessageType, BybitTickerUpdate}; // Add BybitTickerUpdate
use crate::exchanges::traits::{ErrorKind, ExchangeError, ExchangeMessage, WebSocketExchange};
use crate::exchanges::Exchange;
use crate::{HftError, Result};
use std::time::Duration;
use tokio::time::{interval, timeout, Instant};
use std::collections::HashMap;

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
    /// Local ticker cache for delta merging
    tickers: HashMap<Symbol, TickerData>,
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
            tickers: HashMap::new(),
        }
    }
    
    /// Create new Bybit client for testnet
    pub fn new_testnet() -> Self {
        let mut client = Self::new();
        client.monitor = ConnectionMonitor::new("bybit-testnet".to_string());
        client
    }

    /// Merge ticker update into cache and return full ticker
    fn merge_ticker(&mut self, update: BybitTickerUpdate) -> Option<TickerData> {
        // Get or create entry
        let ticker = self.tickers.entry(update.symbol).or_insert_with(|| TickerData {
            symbol: update.symbol,
            bid_price: FixedPoint8::ZERO,
            ask_price: FixedPoint8::ZERO,
            bid_qty: FixedPoint8::ZERO,
            ask_qty: FixedPoint8::ZERO,
            timestamp: 0,
        });
        
        // Update fields
        if let Some(p) = update.bid_price { ticker.bid_price = p; }
        if let Some(q) = update.bid_qty { ticker.bid_qty = q; }
        if let Some(p) = update.ask_price { ticker.ask_price = p; }
        if let Some(q) = update.ask_qty { ticker.ask_qty = q; }
        if update.timestamp > ticker.timestamp { ticker.timestamp = update.timestamp; }
        
        // Return copy if valid (has prices)
        if ticker.bid_price.is_positive() && ticker.ask_price.is_positive() {
            Some(*ticker)
        } else {
            None
        }
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
    pub async fn subscribe_public_trades(&mut self, symbols: &[Symbol]) -> Result<()> {
        if symbols.is_empty() {
            return Ok(());
        }

        // Request subscription
        self.subscriptions.request_subscription(symbols, StreamType::Trade);
        
        // Create topics for V5 protocol
        let topics: Vec<String> = symbols
            .iter()
            .map(|s| {
                let name = SymbolMapper::get_name(*s, Exchange::Bybit).unwrap_or(s.as_str());
                format!("publicTrade.{}", name)
            })
            .collect();
        
        // Send V5 subscription message
        let subscribe_msg = serde_json::json!({
            "op": "subscribe",
            "args": topics,
        });
        
        if let Some(conn) = self.connection.as_mut() {
            conn.send_text(&subscribe_msg.to_string())
                .await
                .map_err(|e| HftError::WebSocket(e.to_string()))?;
        }
        
        Ok(())
    }

    /// Subscribe to ticker stream for symbols
    pub async fn subscribe_tickers(&mut self, symbols: &[Symbol]) -> Result<()> {
        if symbols.is_empty() {
            return Ok(());
        }

        self.subscriptions.request_subscription(symbols, StreamType::Ticker);
        
        let topics: Vec<String> = symbols
            .iter()
            .map(|s| {
                let name = SymbolMapper::get_name(*s, Exchange::Bybit).unwrap_or(s.as_str());
                format!("tickers.{}", name)
            })
            .collect();
        
        let subscribe_msg = serde_json::json!({
            "op": "subscribe",
            "args": topics,
        });
        
        if let Some(conn) = self.connection.as_mut() {
            conn.send_text(&subscribe_msg.to_string())
                .await
                .map_err(|e| HftError::WebSocket(e.to_string()))?;
        }
        
        Ok(())
    }

    /// Subscribe to orderbook stream for symbols
    pub async fn subscribe_orderbook(&mut self, symbols: &[Symbol]) -> Result<()> {
        if symbols.is_empty() {
            return Ok(());
        }

        self.subscriptions.request_subscription(symbols, StreamType::OrderBook);
        
        let topics: Vec<String> = symbols
            .iter()
            .map(|s| {
                let name = SymbolMapper::get_name(*s, Exchange::Bybit).unwrap_or(s.as_str());
                format!("orderbook.1.{}", name)
            })
            .collect();
        
        let subscribe_msg = serde_json::json!({
            "op": "subscribe",
            "args": topics,
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
            loop {
                // Send ping if inactive for 20s
                if self.last_message.elapsed() > Duration::from_secs(20) {
                    let ping_msg = serde_json::json!({"op": "ping"});
                    if let Err(e) = conn.send_text(&ping_msg.to_string()).await {
                        return Err(HftError::WebSocket(e.to_string()));
                    }
                    self.last_message = Instant::now(); 
                }

                // Wait for message with timeout to allow ping check
                match timeout(Duration::from_secs(5), conn.recv()).await {
                    Ok(Ok(Some(msg))) => {
                        self.last_message = Instant::now();
                        self.monitor.record_activity();
                        
                        if let Ok(text) = msg.to_text() {
                            match Self::parse_message_static(text) {
                                Ok(Some(parsed)) => return Ok(Some(parsed)),
                                Ok(None) => {
                                    tracing::debug!("Ignored Bybit msg: {}", text);
                                    continue;
                                },
                                Err(e) => {
                                    tracing::warn!("Parse error: {}", e);
                                    continue;
                                }
                            }
                        }
                    }
                    Ok(Ok(None)) => {
                        self.connection = None;
                        return Ok(None);
                    }
                    Ok(Err(e)) => {
                        return Err(HftError::WebSocket(e.to_string()));
                    }
                    Err(_) => {
                        // Timeout, loop again to check ping
                        continue;
                    }
                }
            }
        }
        
        Ok(None)
    }

    /// Parse Bybit V5 message (static)
    fn parse_message_static(text: &str) -> Result<Option<BybitMessage>> {
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
                match BybitParser::parse_ticker_update(data) {
                    Some(result) => Ok(Some(BybitMessage::TickerUpdate(result.data))),
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

    /// Parse Bybit V5 message
    fn parse_message(&mut self, text: &str) -> Result<Option<BybitMessage>> {
        Self::parse_message_static(text)
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
                // Should not happen for V5 linear (deltas only), but support it
                Ok(Some(ExchangeMessage::Ticker(Exchange::Bybit, ticker)))
            }
            Some(BybitMessage::TickerUpdate(update)) => {
                if let Some(ticker) = self.merge_ticker(update) {
                    Ok(Some(ExchangeMessage::Ticker(Exchange::Bybit, ticker)))
                } else {
                    // Update processed but ticker not yet valid/complete
                    Ok(None)
                }
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
    /// Ticker data (full snapshot)
    Ticker(TickerData),
    /// Ticker update (delta)
    TickerUpdate(BybitTickerUpdate),
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
