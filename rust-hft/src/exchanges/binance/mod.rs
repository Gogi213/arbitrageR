//! Binance Futures WebSocket client
//!
//! Native WebSocket client for Binance Futures exchange.
//! Handles aggTrade and bookTicker streams.

use crate::core::{FixedPoint8, Side, Symbol, TickerData, TradeData};
use crate::ws::connection::{WebSocketConnection, WebSocketError};
use crate::ws::subscription::{StreamType, SubscriptionManager};
use crate::ws::ping::{PingHandler, ConnectionMonitor};
use crate::exchanges::parsing::{BinanceParser, BinanceMessageType};
use crate::HftError;

use std::time::Duration;
use tokio::time::{interval, Instant};

/// Binance Futures WebSocket client
pub struct BinanceWsClient {
    /// WebSocket connection
    connection: Option<WebSocketConnection>,
    /// Subscription manager
    subscriptions: SubscriptionManager,
    /// Connection monitor (ping/pong)
    monitor: ConnectionMonitor,
    /// Last message timestamp
    last_message: Instant,
}

impl BinanceWsClient {
    /// Binance Futures WebSocket URL
    pub const WS_URL: &'static str = "wss://fstream.binance.com/ws";
    
    /// Create new Binance client
    pub fn new() -> Self {
        Self {
            connection: None,
            subscriptions: SubscriptionManager::new(),
            monitor: ConnectionMonitor::new("binance".to_string()),
            last_message: Instant::now(),
        }
    }

    /// Connect to Binance WebSocket
    pub async fn connect(&mut self) -> Result<(), HftError> {
        let mut conn = WebSocketConnection::connect(Self::WS_URL)
            .await
            .map_err(|e| HftError::WebSocket(e.to_string()))?;
        
        self.monitor = ConnectionMonitor::new("binance".to_string());
        self.connection = Some(conn);
        
        Ok(())
    }

    /// Subscribe to aggTrade stream for symbols
    pub async fn subscribe_agg_trades(&mut self, symbols: &[Symbol]) -> Result<(), HftError> {
        if symbols.is_empty() {
            return Ok(());
        }

        // Request subscription
        self.subscriptions.request_subscription(symbols, StreamType::Trade);
        
        // Create batch subscription message
        let batches = self.subscriptions.create_batches(StreamType::Trade);
        
        for batch in batches {
            let stream_names: Vec<String> = batch.symbols
                .iter()
                .map(|s| format!("{}{}", s.as_str().to_lowercase(), "@aggTrade"))
                .collect();
            
            let subscribe_msg = format!(
                "{{\"method\":\"SUBSCRIBE\",\"params\":{},\"id\":1}}",
                serde_json::to_string(&stream_names).unwrap_or_default()
            );
            
            if let Some(conn) = self.connection.as_mut() {
                conn.send_text(&subscribe_msg).await
                    .map_err(|e| HftError::WebSocket(e.to_string()))?;
            }
        }
        
        Ok(())
    }

    /// Subscribe to bookTicker stream for symbols
    pub async fn subscribe_book_tickers(&mut self, symbols: &[Symbol]) -> Result<(), HftError> {
        if symbols.is_empty() {
            return Ok(());
        }

        self.subscriptions.request_subscription(symbols, StreamType::Ticker);
        
        let batches = self.subscriptions.create_batches(StreamType::Ticker);
        
        for batch in batches {
            let stream_names: Vec<String> = batch.symbols
                .iter()
                .map(|s| format!("{}{}", s.as_str().to_lowercase(), "@bookTicker"))
                .collect();
            
            let subscribe_msg = format!(
                "{{\"method\":\"SUBSCRIBE\",\"params\":{},\"id\":1}}",
                serde_json::to_string(&stream_names).unwrap_or_default()
            );
            
            if let Some(conn) = self.connection.as_mut() {
                conn.send_text(&subscribe_msg).await
                    .map_err(|e| HftError::WebSocket(e.to_string()))?;
            }
        }
        
        Ok(())
    }

    /// Receive and process next message
    pub async fn recv(&mut self) -> Result<Option<BinanceMessage>, HftError> {
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

    /// Parse Binance message into structured data
    fn parse_message(
        &mut self,
        text: &str,
    ) -> Result<Option<BinanceMessage>, HftError> {
        let data = text.as_bytes();
        
        // Detect message type and parse accordingly
        match BinanceParser::detect_message_type(data) {
            BinanceMessageType::AggTrade => {
                match BinanceParser::parse_trade(data) {
                    Some(result) => Ok(Some(BinanceMessage::Trade(result.data))),
                    None => Ok(None),
                }
            }
            BinanceMessageType::BookTicker => {
                match BinanceParser::parse_ticker(data) {
                    Some(result) => Ok(Some(BinanceMessage::Ticker(result.data))),
                    None => Ok(None),
                }
            }
            BinanceMessageType::SubscriptionResponse => {
                Ok(Some(BinanceMessage::SubscriptionConfirmed))
            }
            BinanceMessageType::Unknown => {
                // Unknown message type, could be heartbeat or error
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

    /// Get active trade subscriptions
    pub fn active_trade_subscriptions(&self) -> Vec<Symbol> {
        self.subscriptions.get_active(StreamType::Trade)
    }

    /// Get active ticker subscriptions  
    pub fn active_ticker_subscriptions(&self) -> Vec<Symbol> {
        self.subscriptions.get_active(StreamType::Ticker)
    }
}

impl Default for BinanceWsClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Binance message types
#[derive(Debug, Clone)]
pub enum BinanceMessage {
    /// Trade/aggTrade data
    Trade(TradeData),
    /// Ticker/bookTicker data
    Ticker(TickerData),
    /// Subscription confirmation
    SubscriptionConfirmed,
    /// Ping/pong
    Heartbeat,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_binance_client_creation() {
        let client = BinanceWsClient::new();
        assert!(!client.is_connected());
    }

    #[test]
    fn test_parse_agg_trade() {
        let client = BinanceWsClient::new();
        
        // Note: This test would need actual JSON parsing
        // For now, just verify the method exists
    }

    #[test]
    fn test_parse_book_ticker() {
        let client = BinanceWsClient::new();
        
        // Note: This test would need actual JSON parsing
    }
}

// TODO Phase 3.3: Implement zero-copy parsing benchmarks
// Benchmark target: <5μs for JSON → TradeData/TickerData
