//! Ultra-low latency arbitrage bot for Binance and Bybit
//! 
//! # Architecture
//! - **core**: Zero-allocation types (FixedPoint8, Symbol, TickerData)
//! - **hot_path**: Latency-critical code (parsing, routing, calculations)
//! - **exchanges**: Exchange-specific implementations
//! - **ws**: WebSocket clients
//! - **rest**: REST API clients
//! - **infrastructure**: Cold path (logging, metrics, config, api)

#![feature(portable_simd)]
#![allow(incomplete_features)]

use rust_hft::hot_path::ThresholdTracker;
use rust_hft::infrastructure::{start_server, metrics::MetricsCollector, config::Config};
use rust_hft::engine::AppEngine;
use rust_hft::exchanges::{BinanceWsClient, BybitWsClient, ExchangeClient};
use rust_hft::core::{Symbol, SymbolDiscovery, SymbolRegistry};
use rust_hft::{HftError, Result};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Main application state
pub struct HftApp {
    /// Configuration (read-heavy, rarely changed)
    config: Arc<RwLock<Config>>,
}

impl HftApp {
    /// Create new application instance
    pub async fn new(config: Config) -> Result<Self> {
        Ok(Self {
            config: Arc::new(RwLock::new(config)),
        })
    }
    
    /// Run the main event loop
    pub async fn run(&self) -> Result<()> {
        tracing::info!("Starting HFT Arbitrage Bot...");
        
        // 1. Initialize Core Components
        let tracker = Arc::new(RwLock::new(ThresholdTracker::new()));
        let metrics = Arc::new(MetricsCollector::new());
        
        // 2. Start API Server (Cold Path)
        let tracker_for_api = tracker.clone();
        let metrics_for_api = metrics.clone();
        let config_guard = self.config.read().await;
        let api_config = config_guard.api.clone();
        drop(config_guard); // Release lock early
        
        tokio::spawn(async move {
            if let Err(e) = start_server(tracker_for_api, metrics_for_api, &api_config).await {
                tracing::error!("API Server failed: {}", e);
            }
        });
        
        // 3. Start AppEngine (Hot Path)
        let mut engine = AppEngine::new(tracker.clone(), metrics.clone());
        
        // Add exchanges
        engine.add_exchange(ExchangeClient::Binance(BinanceWsClient::new()));
        engine.add_exchange(ExchangeClient::Bybit(BybitWsClient::new()));
        
        // 4. Discover liquid symbols dynamically (Cold Path - startup only)
        tracing::info!("Discovering liquid symbols from exchanges...");
        
        // Step 1: Fetch symbol names
        let discovery = SymbolDiscovery::new();
        let names = discovery.fetch_symbol_names().await
            .map_err(|e| HftError::RestApi(format!("Failed to fetch symbol names: {}", e)))?;
        tracing::info!("Fetched {} symbol names", names.len());
        
        // Step 2: Register symbols in global registry
        SymbolRegistry::initialize(&names)
            .map_err(|e| HftError::Config(format!("Failed to initialize symbol registry: {}", e)))?;
        
        // Step 3: Fetch full data with registered symbols
        let discovered = discovery.fetch_all_liquid().await
            .map_err(|e| HftError::RestApi(format!("Failed to fetch liquid symbols: {}", e)))?;
        
        let symbols: Vec<Symbol> = discovered.into_iter()
            .map(|d| d.symbol)
            .collect();
        tracing::info!("Discovered {} liquid symbols", symbols.len());
        
        // Run engine (this blocks the task)
        engine.run(&symbols).await?;
        
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing for cold path logging
    tracing_subscriber::fmt::init();
    
    // Load config or use defaults
    let config = Config::load().unwrap_or_default();
    
    let app = HftApp::new(config).await?;
    app.run().await?;
    
    Ok(())
}
