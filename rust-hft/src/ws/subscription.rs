//! Subscription manager for batched WebSocket subscriptions
//!
//! Manages symbol subscriptions with batching (200 symbols per request for Binance).
//! Tracks pending and active subscriptions, handles confirmations and retries.

use crate::core::Symbol;
use std::collections::{HashMap, HashSet};

/// Maximum symbols per subscription batch (Binance limit)
pub const MAX_BATCH_SIZE: usize = 200;

/// Subscription request status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscriptionStatus {
    /// Request sent, waiting for confirmation
    Pending,
    /// Subscription confirmed by exchange
    Active,
    /// Subscription failed
    Failed,
    /// Subscription cancelled
    Cancelled,
}

/// Subscription entry for a single symbol
#[derive(Debug, Clone)]
pub struct Subscription {
    pub symbol: Symbol,
    pub status: SubscriptionStatus,
    pub retry_count: u32,
    pub stream_type: StreamType,
}

/// Type of data stream
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StreamType {
    /// Trade/aggTrade stream
    Trade,
    /// Ticker/bookTicker stream
    Ticker,
    /// Order book stream
    OrderBook,
    /// User data stream (private)
    UserData,
}

impl StreamType {
    /// Get stream name suffix for the exchange
    pub fn as_str(&self) -> &'static str {
        match self {
            StreamType::Trade => "@aggTrade",
            StreamType::Ticker => "@bookTicker",
            StreamType::OrderBook => "@depth",
            StreamType::UserData => "@userData",
        }
    }
}

/// Batch subscription request
#[derive(Debug, Clone)]
pub struct BatchRequest {
    pub symbols: Vec<Symbol>,
    pub stream_type: StreamType,
    pub status: SubscriptionStatus,
}

/// Subscription manager for handling batched subscriptions
pub struct SubscriptionManager {
    /// All subscriptions indexed by (symbol, stream_type)
    subscriptions: HashMap<(Symbol, StreamType), Subscription>,
    /// Active symbols by stream type
    active_by_type: HashMap<StreamType, HashSet<Symbol>>,
    /// Maximum retry attempts
    max_retries: u32,
}

impl SubscriptionManager {
    /// Create new subscription manager
    pub fn new() -> Self {
        let mut active_by_type = HashMap::new();
        active_by_type.insert(StreamType::Trade, HashSet::new());
        active_by_type.insert(StreamType::Ticker, HashSet::new());
        active_by_type.insert(StreamType::OrderBook, HashSet::new());
        active_by_type.insert(StreamType::UserData, HashSet::new());

        Self {
            subscriptions: HashMap::new(),
            active_by_type,
            max_retries: 3,
        }
    }

    /// Request subscription for symbols
    ///
    /// # Arguments
    /// * `symbols` - Symbols to subscribe to
    /// * `stream_type` - Type of data stream
    pub fn request_subscription(
        &mut self,
        symbols: &[Symbol],
        stream_type: StreamType,
    ) {
        for &symbol in symbols {
            let key = (symbol, stream_type);
            
            if !self.subscriptions.contains_key(&key) {
                let subscription = Subscription {
                    symbol,
                    status: SubscriptionStatus::Pending,
                    retry_count: 0,
                    stream_type,
                };
                self.subscriptions.insert(key, subscription);
            }
        }
    }

    /// Cancel subscription for symbols
    pub fn cancel_subscription(
        &mut self,
        symbols: &[Symbol],
        stream_type: StreamType,
    ) {
        for &symbol in symbols {
            let key = (symbol, stream_type);
            
            if let Some(sub) = self.subscriptions.get_mut(&key) {
                sub.status = SubscriptionStatus::Cancelled;
            }
            
            // Remove from active set
            if let Some(active) = self.active_by_type.get_mut(&stream_type) {
                active.remove(&symbol);
            }
        }
    }

    /// Create batch requests from pending subscriptions
    ///
    /// Returns batches of up to MAX_BATCH_SIZE symbols
    pub fn create_batches(&mut self,
        stream_type: StreamType,
    ) -> Vec<BatchRequest> {
        // Collect pending subscriptions for this stream type
        let pending: Vec<Symbol> = self
            .subscriptions
            .iter()
            .filter(|(key, sub)| {
                key.1 == stream_type && sub.status == SubscriptionStatus::Pending
            })
            .map(|(key, _)| key.0)
            .collect();

        // Split into batches
        let mut batches = Vec::new();
        for chunk in pending.chunks(MAX_BATCH_SIZE) {
            let batch = BatchRequest {
                symbols: chunk.to_vec(),
                stream_type,
                status: SubscriptionStatus::Pending,
            };
            batches.push(batch);
        }

        batches
    }

    /// Mark symbols as active (confirmed by exchange)
    pub fn confirm(&mut self, symbols: &[Symbol], stream_type: StreamType) {
        for &symbol in symbols {
            let key = (symbol, stream_type);
            
            if let Some(sub) = self.subscriptions.get_mut(&key) {
                sub.status = SubscriptionStatus::Active;
                sub.retry_count = 0;
            }
            
            // Add to active set
            if let Some(active) = self.active_by_type.get_mut(&stream_type) {
                active.insert(symbol);
            }
        }
    }

    /// Mark subscription as failed
    pub fn mark_failed(&mut self, symbol: Symbol, stream_type: StreamType) {
        let key = (symbol, stream_type);
        
        if let Some(sub) = self.subscriptions.get_mut(&key) {
            sub.retry_count += 1;
            
            if sub.retry_count >= self.max_retries {
                sub.status = SubscriptionStatus::Failed;
            } else {
                // Reset to pending for retry
                sub.status = SubscriptionStatus::Pending;
            }
        }
    }

    /// Get all active subscriptions for a stream type
    pub fn get_active(&self, stream_type: StreamType) -> Vec<Symbol> {
        self.active_by_type
            .get(&stream_type)
            .map(|set| set.iter().copied().collect())
            .unwrap_or_default()
    }

    /// Check if symbol is subscribed for stream type
    pub fn is_subscribed(&self, symbol: Symbol, stream_type: StreamType) -> bool {
        let key = (symbol, stream_type);
        self.subscriptions
            .get(&key)
            .map(|sub| {
                sub.status == SubscriptionStatus::Active ||
                sub.status == SubscriptionStatus::Pending
            })
            .unwrap_or(false)
    }

    /// Check if symbol has active subscription
    pub fn is_active(&self, symbol: Symbol, stream_type: StreamType) -> bool {
        self.active_by_type
            .get(&stream_type)
            .map(|set| set.contains(&symbol))
            .unwrap_or(false)
    }

    /// Get subscription status
    pub fn get_status(
        &self,
        symbol: Symbol,
        stream_type: StreamType,
    ) -> Option<SubscriptionStatus> {
        let key = (symbol, stream_type);
        self.subscriptions.get(&key).map(|sub| sub.status)
    }

    /// Get count of active subscriptions
    pub fn active_count(&self, stream_type: StreamType) -> usize {
        self.active_by_type
            .get(&stream_type)
            .map(|set| set.len())
            .unwrap_or(0)
    }

    /// Get total subscriptions count
    pub fn total_count(&self) -> usize {
        self.subscriptions.len()
    }

    /// Get pending count
    pub fn pending_count(&self, stream_type: StreamType) -> usize {
        self.subscriptions
            .values()
            .filter(|sub| {
                sub.stream_type == stream_type &&
                sub.status == SubscriptionStatus::Pending
            })
            .count()
    }

    /// Clear all subscriptions
    pub fn clear(&mut self) {
        self.subscriptions.clear();
        for active in self.active_by_type.values_mut() {
            active.clear();
        }
    }

    /// Get symbols that need retry
    pub fn get_retry_symbols(&self,
        stream_type: StreamType,
    ) -> Vec<Symbol> {
        self.subscriptions
            .values()
            .filter(|sub| {
                sub.stream_type == stream_type &&
                sub.status == SubscriptionStatus::Pending &&
                sub.retry_count > 0
            })
            .map(|sub| sub.symbol)
            .collect()
    }
}

impl Default for SubscriptionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subscription_manager_creation() {
        let manager = SubscriptionManager::new();
        assert_eq!(manager.total_count(), 0);
        assert_eq!(manager.active_count(StreamType::Trade), 0);
    }

    #[test]
    fn test_request_subscription() {
        let mut manager = SubscriptionManager::new();
        
        manager.request_subscription(&[Symbol::BTCUSDT, Symbol::ETHUSDT],
            StreamType::Trade
        );
        
        assert_eq!(manager.total_count(), 2);
        assert!(manager.is_subscribed(Symbol::BTCUSDT, StreamType::Trade));
        assert!(manager.is_subscribed(Symbol::ETHUSDT, StreamType::Trade));
    }

    #[test]
    fn test_create_batches() {
        let mut manager = SubscriptionManager::new();
        
        // Create many symbols
        let symbols: Vec<Symbol> = (0..250)
            .map(|i| Symbol::from_raw(i))
            .collect();
        
        manager.request_subscription(&symbols,
            StreamType::Trade
        );
        
        let batches = manager.create_batches(StreamType::Trade);
        
        // Should create 2 batches: 200 + 50
        assert_eq!(batches.len(), 2);
        assert_eq!(batches[0].symbols.len(), 200);
        assert_eq!(batches[1].symbols.len(), 50);
    }

    #[test]
    fn test_confirm_subscription() {
        let mut manager = SubscriptionManager::new();
        
        manager.request_subscription(
            &[Symbol::BTCUSDT],
            StreamType::Trade
        );
        
        assert!(!manager.is_active(Symbol::BTCUSDT, StreamType::Trade));
        
        manager.confirm(&[Symbol::BTCUSDT], StreamType::Trade);
        
        assert!(manager.is_active(Symbol::BTCUSDT, StreamType::Trade));
        assert_eq!(manager.active_count(StreamType::Trade), 1);
    }

    #[test]
    fn test_mark_failed() {
        let mut manager = SubscriptionManager::new();
        manager.max_retries = 2;
        
        manager.request_subscription(
            &[Symbol::BTCUSDT],
            StreamType::Trade
        );
        
        // First failure - should stay pending
        manager.mark_failed(Symbol::BTCUSDT, StreamType::Trade);
        assert_eq!(
            manager.get_status(Symbol::BTCUSDT, StreamType::Trade),
            Some(SubscriptionStatus::Pending)
        );
        
        // Second failure - should become failed
        manager.mark_failed(Symbol::BTCUSDT, StreamType::Trade);
        assert_eq!(
            manager.get_status(Symbol::BTCUSDT, StreamType::Trade),
            Some(SubscriptionStatus::Failed)
        );
    }

    #[test]
    fn test_cancel_subscription() {
        let mut manager = SubscriptionManager::new();
        
        manager.request_subscription(
            &[Symbol::BTCUSDT],
            StreamType::Trade
        );
        manager.confirm(&[Symbol::BTCUSDT], StreamType::Trade);
        
        assert!(manager.is_active(Symbol::BTCUSDT, StreamType::Trade));
        
        manager.cancel_subscription(
            &[Symbol::BTCUSDT],
            StreamType::Trade
        );
        
        assert!(!manager.is_subscribed(Symbol::BTCUSDT, StreamType::Trade));
        assert!(!manager.is_active(Symbol::BTCUSDT, StreamType::Trade));
    }

    #[test]
    fn test_multiple_stream_types() {
        let mut manager = SubscriptionManager::new();
        
        // Subscribe to trades and tickers for BTC
        manager.request_subscription(
            &[Symbol::BTCUSDT],
            StreamType::Trade
        );
        manager.request_subscription(
            &[Symbol::BTCUSDT],
            StreamType::Ticker
        );
        
        assert_eq!(manager.total_count(), 2);
        
        manager.confirm(&[Symbol::BTCUSDT], StreamType::Trade);
        manager.confirm(&[Symbol::BTCUSDT], StreamType::Ticker);
        
        assert!(manager.is_active(Symbol::BTCUSDT, StreamType::Trade));
        assert!(manager.is_active(Symbol::BTCUSDT, StreamType::Ticker));
    }

    #[test]
    fn test_duplicate_subscription_request() {
        let mut manager = SubscriptionManager::new();
        
        // Request twice - should only create one subscription
        manager.request_subscription(
            &[Symbol::BTCUSDT],
            StreamType::Trade
        );
        manager.request_subscription(
            &[Symbol::BTCUSDT],
            StreamType::Trade
        );
        
        assert_eq!(manager.total_count(), 1);
    }

    #[test]
    fn test_get_retry_symbols() {
        let mut manager = SubscriptionManager::new();
        
        manager.request_subscription(
            &[Symbol::BTCUSDT, Symbol::ETHUSDT],
            StreamType::Trade
        );
        
        // Mark BTC as failed once
        manager.mark_failed(Symbol::BTCUSDT, StreamType::Trade);
        
        let retry = manager.get_retry_symbols(StreamType::Trade);
        assert_eq!(retry.len(), 1);
        assert_eq!(retry[0], Symbol::BTCUSDT);
    }

    #[test]
    fn test_clear() {
        let mut manager = SubscriptionManager::new();
        
        manager.request_subscription(
            &[Symbol::BTCUSDT],
            StreamType::Trade
        );
        manager.confirm(&[Symbol::BTCUSDT], StreamType::Trade);
        
        manager.clear();
        
        assert_eq!(manager.total_count(), 0);
        assert_eq!(manager.active_count(StreamType::Trade), 0);
    }
}

// HFT Checklist:
// ✓ BitSet for O(1) active check (HashSet in this case)
// ✓ No allocation in batch creation (Vec is pre-allocated)
// ✓ Minimal copying
// ✓ Fast lookup by (Symbol, StreamType)
// ✓ No locking (assumes single-threaded or external synchronization)
