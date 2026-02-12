//! Ultra-low latency HFT arbitrage bot
//!
//! Core library for zero-allocation parsing and trading operations.

pub mod core;
pub mod exchanges;
pub mod hot_path;
pub mod infrastructure;
pub mod rest;
pub mod ws;
pub mod engine;

#[cfg(test)]
pub mod test_utils;

// Re-export commonly used types
pub use infrastructure::config::{Config, HftConfig, ApiConfig};

use thiserror::Error;

/// Main error type for the HFT bot
#[derive(Error, Debug)]
pub enum HftError {
    #[error("WebSocket error: {0}")]
    WebSocket(String),

    #[error("REST API error: {0}")]
    RestApi(String),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Result type alias
pub type Result<T> = std::result::Result<T, HftError>;