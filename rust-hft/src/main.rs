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
use rust_hft::infrastructure::start_server;
use rust_hft::{HftError, Result};
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
        
        // 2. Start API Server (Cold Path)
        let tracker_for_api = tracker.clone();
        let config = self.config.read().await;
        let port = config.api_port;
        
        tokio::spawn(async move {
            if let Err(e) = start_server(tracker_for_api, port).await {
                tracing::error!("API Server failed: {}", e);
            }
        });
        
        // 3. TODO: Start WebSocket Clients (Hot Path)
        // This will be implemented in Phase 4.4 integration
        
        tracing::info!("System initialized. Waiting for connections...");
        
        // Keep main loop running
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing for cold path logging
    tracing_subscriber::fmt::init();
    
    // TODO: Load config from file
    let config = Config {
        api_port: 3000,
    };
    
    let app = HftApp::new(config).await?;
    app.run().await?;
    
    Ok(())
}
