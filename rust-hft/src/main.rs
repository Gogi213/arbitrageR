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
use rust_hft::infrastructure::{start_server, metrics::MetricsCollector};
use rust_hft::engine::AppEngine;
use rust_hft::exchanges::{BinanceWsClient, BybitWsClient, ExchangeClient};
use rust_hft::core::{Symbol, SymbolDiscovery, SymbolRegistry};
use rust_hft::Result;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Main application state
pub struct HftApp {
    /// Configuration (read-heavy, rarely changed)
    config: Arc<RwLock<Config>>,
}

/// Application configuration
#[derive(Debug, Clone)]
pub struct Config {
    pub api_port: u16,
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
        let config = self.config.read().await;
        let port = config.api_port;
        
        tokio::spawn(async move {
            if let Err(e) = start_server(tracker_for_api, metrics_for_api, port).await {
                tracing::error!("API Server failed: {}", e);
            }
        });
        
        // 3. Start AppEngine (Hot Path)
        let mut engine = AppEngine::new(tracker.clone(), metrics.clone());
        
        // Add exchanges
        engine.add_exchange(ExchangeClient::Binance(BinanceWsClient::new()));
        engine.add_exchange(ExchangeClient::Bybit(BybitWsClient::new()));
        
        // 4. Discover liquid symbols dynamically (Cold Path - startup only)
        println!("DEBUG: Starting discovery...");
        tracing::info!("Discovering liquid symbols from exchanges...");
        
        let symbols = if !SymbolRegistry::is_initialized() {
            // Step 1: Fetch symbol names only (without parsing)
            println!("DEBUG: Registry not initialized, creating discovery...");
            let discovery = SymbolDiscovery::new();
            println!("DEBUG: Fetching symbol names...");
            match discovery.fetch_symbol_names().await {
                Ok(names) => {
                    println!("DEBUG: Fetched {} symbol names", names.len());
                    tracing::info!("Fetched {} symbol names", names.len());
                    
                    // Step 2: Register symbols in global registry
                    if let Err(e) = SymbolRegistry::initialize(&names) {
                        tracing::warn!("Failed to initialize symbol registry: {}. Using fallback.", e);
                        vec![
                            Symbol::BTCUSDT,
                            Symbol::ETHUSDT,
                            Symbol::SOLUSDT,
                            Symbol::BNBUSDT,
                            Symbol::XRPUSDT,
                        ]
                    } else {
                        // Step 3: Now fetch full data with registered symbols
                        match discovery.fetch_all_liquid().await {
                            Ok(discovered) => {
                                let symbols: Vec<Symbol> = discovered.into_iter()
                                    .map(|d| d.symbol)
                                    .collect();
                                tracing::info!("Discovered {} liquid symbols after registration", symbols.len());
                                symbols
                            }
                            Err(e) => {
                                tracing::warn!("Failed to fetch liquid symbols: {}. Using fallback.", e);
                                vec![
                                    Symbol::BTCUSDT,
                                    Symbol::ETHUSDT,
                                    Symbol::SOLUSDT,
                                    Symbol::BNBUSDT,
                                    Symbol::XRPUSDT,
                                ]
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to fetch symbol names: {}. Using fallback.", e);
                    // Fallback to major pairs
                    vec![
                        Symbol::BTCUSDT,
                        Symbol::ETHUSDT,
                        Symbol::SOLUSDT,
                        Symbol::BNBUSDT,
                        Symbol::XRPUSDT,
                    ]
                }
            }
        } else {
            // Registry already initialized
            let discovery = SymbolDiscovery::new();
            match discovery.fetch_all_liquid().await {
                Ok(discovered) => discovered.into_iter().map(|d| d.symbol).collect(),
                Err(_) => vec![
                    Symbol::BTCUSDT,
                    Symbol::ETHUSDT,
                    Symbol::SOLUSDT,
                    Symbol::BNBUSDT,
                    Symbol::XRPUSDT,
                ],
            }
        };
        
        // Run engine (this blocks the task)
        engine.run(&symbols).await?;
        
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing for cold path logging
    tracing_subscriber::fmt::init();
    
    // TODO: Load config from file
    let config = Config {
        api_port: 5000,
    };
    
    let app = HftApp::new(config).await?;
    app.run().await?;
    
    Ok(())
}
