//! Core Application Engine
//!
//! Orchestrates WebSocket clients, message routing, and state management.
//! Connects Hot Path (exchanges) to Warm Path (tracker) and Cold Path (API).

use crate::core::Symbol;
use crate::exchanges::{ExchangeClient, ExchangeMessage, Exchange};
use crate::hot_path::ThresholdTracker;
use crate::infrastructure::metrics::MetricsCollector;
use crate::Result;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Main engine managing the trading lifecycle
pub struct AppEngine {
    tracker: Arc<RwLock<ThresholdTracker>>,
    metrics: Arc<MetricsCollector>,
    exchanges: Vec<ExchangeClient>,
    running: bool,
}

impl AppEngine {
    /// Create new engine with shared tracker and metrics
    pub fn new(tracker: Arc<RwLock<ThresholdTracker>>, metrics: Arc<MetricsCollector>) -> Self {
        Self {
            tracker,
            metrics,
            exchanges: Vec::new(),
            running: false,
        }
    }

    /// Get metrics collector reference
    pub fn metrics(&self) -> Arc<MetricsCollector> {
        self.metrics.clone()
    }

    /// Add exchange client
    pub fn add_exchange(&mut self, exchange: ExchangeClient) {
        self.exchanges.push(exchange);
    }

    /// Start the engine and all components
    pub async fn run(&mut self, symbols: &[Symbol]) -> Result<()> {
        if self.running {
            return Ok(());
        }
        self.running = true;

        tracing::info!("Starting AppEngine with {} exchanges", self.exchanges.len());

        // 1. Connect and Subscribe
        for exchange in &mut self.exchanges {
            let name = exchange.name();
            tracing::info!("Connecting to {}...", name);
            
            if let Err(e) = exchange.connect().await {
                tracing::error!("Failed to connect to {}: {}", name, e);
                return Err(e);
            }
            
            // Update connection status in metrics
            if name == "binance" {
                self.metrics.set_binance_connected(true);
            } else if name == "bybit" {
                self.metrics.set_bybit_connected(true);
            }
            
            tracing::info!("Subscribing to {} tickers on {}...", symbols.len(), name);
            if let Err(e) = exchange.subscribe_tickers(symbols).await {
                tracing::error!("Failed to subscribe on {}: {}", name, e);
                return Err(e);
            }
        }

        // 2. Start Message Processing Loop
        // We need to poll multiple exchanges concurrently.
        // Since we have a Vec of mutable clients, we can't easily iterate and await in a single loop
        // without ownership issues or complex polling.
        // Easiest way: Spawn a task for each exchange that feeds a central channel, 
        // OR run a select loop if clients support it.
        // Our clients have `next_message()` which is async.
        
        // Better approach for HFT: Each exchange runs in its own task and updates the tracker directly?
        // But tracker is protected by RwLock.
        // Or send messages to a MPSC channel, and a single thread updates the tracker (Actor model).
        // This avoids lock contention on the tracker.
        
        // Let's use MPSC channel for aggregation.
        let (tx, mut rx) = tokio::sync::mpsc::channel(1024);
        
        let mut handles = Vec::new();
        
        // Take exchanges out of self to move into tasks
        let exchanges = std::mem::take(&mut self.exchanges);
        
        for mut exchange in exchanges {
            let tx = tx.clone();
            let name = exchange.name().to_string();
            
            let handle = tokio::spawn(async move {
                tracing::info!("Started message loop for {}", name);
                loop {
                    match exchange.next_message().await {
                        Ok(Some(msg)) => {
                            if tx.send(msg).await.is_err() {
                                break; // Receiver dropped
                            }
                        }
                        Ok(None) => {
                            tracing::warn!("{} connection closed gracefully", name);
                            break;
                        }
                        Err(e) => {
                            tracing::error!("{} error: {}", name, e);
                            // Simple reconnection logic could go here
                            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                        }
                    }
                }
            });
            handles.push(handle);
        }
        
        // Restore exchanges? No, they are moved. AppEngine effectively hands them off.
        // If we want to stop gracefully, we need a kill signal.
        
        // 3. Process Aggregated Messages
        tracing::info!("Engine running. Processing messages...");
        
        while let Some(msg) = rx.recv().await {
            tracing::debug!("Engine received message: {:?}", msg);
            match msg {
                ExchangeMessage::Ticker(exchange, ticker) => {
                    tracing::info!("Ticker received: {:?} from {:?}", ticker, exchange);
                    // Record metrics (cold path - don't block hot path)
                    match exchange {
                        Exchange::Binance => self.metrics.record_binance_message(),
                        Exchange::Bybit => self.metrics.record_bybit_message(),
                    }
                    
                    // Update tracker (Warm Path)
                    let mut tracker = self.tracker.write().await;
                    if let Some(event) = tracker.update(ticker, exchange) {
                        // Log significant spreads
                        if event.spread.as_raw() > 50_000 { // > 0.05%
                            tracing::info!(
                                "OPPORTUNITY: {} {:.4}% Buy {:?} Sell {:?}", 
                                event.symbol.as_str(),
                                event.spread.to_f64() * 100.0,
                                event.long_ex,
                                event.short_ex
                            );
                        } else {
                            tracing::debug!("Spread updated: {} {:.4}%", event.symbol.as_str(), event.spread.to_f64() * 100.0);
                        }
                    } else {
                        tracing::debug!("No arbitrage opportunity for this tick");
                    }
                }
                ExchangeMessage::Trade(exchange, _trade) => {
                    tracing::debug!("Trade received from {:?}", exchange);
                    match exchange {
                        Exchange::Binance => self.metrics.record_binance_message(),
                        Exchange::Bybit => self.metrics.record_bybit_message(),
                    }
                }
                ExchangeMessage::Heartbeat => {
                    // Heartbeat received - connection alive
                    tracing::debug!("Heartbeat received");
                }
                ExchangeMessage::Error(e) => {
                    tracing::error!("Exchange error: [{:?}] {}", e.exchange, e.message);
                }
            }
        }
        
        Ok(())
    }
}
