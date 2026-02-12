//! Binance Futures WebSocket client
//!
//! Native WebSocket client for Binance Futures exchange.
//! Handles aggTrade and bookTicker streams.

use crate::core::{Symbol, TickerData, TradeData, SymbolMapper};
use crate::ws::connection::WebSocketConnection;
use crate::ws::subscription::{StreamType, SubscriptionManager};
use crate::ws::ping::ConnectionMonitor;
use crate::exchanges::parsing::{BinanceParser, BinanceMessageType};
use crate::exchanges::traits::{ExchangeMessage, WebSocketExchange};
use crate::exchanges::Exchange;
use crate::{HftError, Result};

use tokio::time::Instant;

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
    pub async fn connect(&mut self) -> Result<()> {
        let conn = WebSocketConnection::connect(Self::WS_URL)
            .await
            .map_err(|e| HftError::WebSocket(e.to_string()))?;
        
        self.monitor = ConnectionMonitor::new("binance".to_string());
        self.connection = Some(conn);
        
        Ok(())
    }

    /// Subscribe to aggTrade stream for symbols
    pub async fn subscribe_agg_trades(&mut self, symbols: &[Symbol]) -> Result<()> {
        if symbols.is_empty() {
            return Ok(());
        }

        // Request subscription
        self.subscriptions.request_subscription(symbols, StreamType::Trade);
        
        // Create batch subscription message
        let batches = self.subscriptions.create_batches(StreamType::Trade);
        
        for batch in batches {
            let params: Vec<String> = batch.symbols.iter()
                .map(|s| {
                    // Use mapper to get exchange-specific name (e.g. 1000PEPEUSDT)
                    let name = SymbolMapper::get_name(*s, Exchange::Binance).unwrap_or(s.as_str());
                    format!("{}@aggTrade", name.to_lowercase())
                })
                .collect();
            
            let request = serde_json::json!({
                "method": "SUBSCRIBE",
                "params": params,
                "id": 1
            });
            
            if let Some(conn) = self.connection.as_mut() {
                conn.send_text(&request.to_string()).await
                    .map_err(|e| HftError::WebSocket(e.to_string()))?;
            }
        }
        
        Ok(())
    }
    
    /// Subscribe to bookTicker stream for symbols
    pub async fn subscribe_book_tickers(&mut self, symbols: &[Symbol]) -> Result<()> {
        if symbols.is_empty() {
            return Ok(());
        }

        self.subscriptions.request_subscription(symbols, StreamType::Ticker);
        
        let batches = self.subscriptions.create_batches(StreamType::Ticker);
        
        for batch in batches {
            let params: Vec<String> = batch.symbols.iter()
                .map(|s| {
                    let name = SymbolMapper::get_name(*s, Exchange::Binance).unwrap_or(s.as_str());
                    format!("{}@bookTicker", name.to_lowercase())
                })
                .collect();
            
            let request = serde_json::json!({
                "method": "SUBSCRIBE",
                "params": params,
                "id": 1
            });
            
            if let Some(conn) = self.connection.as_mut() {
                conn.send_text(&request.to_string()).await
                    .map_err(|e| HftError::WebSocket(e.to_string()))?;
            }
        }
        
        Ok(())
    }

    /// Receive and process next message
    pub async fn recv(&mut self) -> Result<Option<BinanceMessage>> {
        if let Some(conn) = self.connection.as_mut() {
            loop {
                match conn.recv().await {
                    Ok(Some(msg)) => {
                        self.last_message = Instant::now();
                        self.monitor.record_activity();
                        
                        // Parse message
                        if let Ok(text) = msg.to_text() {
                            match Self::parse_message(text) {
                                Ok(Some(parsed)) => return Ok(Some(parsed)),
                                Ok(None) => continue, // Unknown message, skip
                                Err(e) => {
                                    tracing::warn!("Parse error: {}", e);
                                    continue;
                                }
                            }
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
        }
        
        Ok(None)
    }

    /// Parse Binance message into structured data
    fn parse_message(
        text: &str,
    ) -> Result<Option<BinanceMessage>> {
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

// === WebSocketExchange Trait Implementation ===

impl WebSocketExchange for BinanceWsClient {
    #[inline]
    fn exchange(&self) -> Exchange {
        Exchange::Binance
    }

    async fn connect(&mut self) -> crate::Result<()> {
        // Use existing connect method
        self.connect().await
    }

    async fn subscribe_trades(&mut self, symbols: &[Symbol]) -> crate::Result<()> {
        self.subscribe_agg_trades(symbols).await
    }

    async fn subscribe_tickers(&mut self, symbols: &[Symbol]) -> crate::Result<()> {
        self.subscribe_book_tickers(symbols).await
    }

    async fn next_message(&mut self) -> crate::Result<Option<ExchangeMessage>> {
        match self.recv().await? {
            Some(BinanceMessage::Trade(trade)) => {
                Ok(Some(ExchangeMessage::Trade(Exchange::Binance, trade)))
            }
            Some(BinanceMessage::Ticker(ticker)) => {
                Ok(Some(ExchangeMessage::Ticker(Exchange::Binance, ticker)))
            }
            Some(BinanceMessage::Heartbeat) => Ok(Some(ExchangeMessage::Heartbeat)),
            Some(BinanceMessage::SubscriptionConfirmed) => {
                // Subscription confirmations don't map to ExchangeMessage
                // Could be treated as Heartbeat or ignored
                Ok(Some(ExchangeMessage::Heartbeat))
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
    }

    #[test]
    fn test_parse_book_ticker() {
        let client = BinanceWsClient::new();
        // Note: This test would need actual JSON parsing
    }
}
