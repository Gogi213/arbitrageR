//! Configuration management for HFT bot
//!
//! Loads configuration from config.toml at startup.
//! All values are configurable to avoid hardcoded constants.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// HFT Configuration
///
/// Loaded from config.toml at startup. Contains all tunable parameters
/// to avoid hardcoded values throughout the codebase.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Config {
    /// HFT-specific settings
    #[serde(default)]
    pub hft: HftConfig,

    /// API server settings
    #[serde(default)]
    pub api: ApiConfig,
}

/// HFT trading configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HftConfig {
    /// Minimum 24h volume to consider a symbol liquid (in USDT)
    #[serde(default = "default_min_volume")]
    pub min_volume_24h: f64,

    /// Opportunity threshold in basis points (FixedPoint8 raw value)
    /// 250_000 = 0.25% spread between exchanges
    #[serde(default = "default_threshold")]
    pub opportunity_threshold_bps: i64,

    /// Rolling window duration in seconds for spread history
    #[serde(default = "default_window_seconds")]
    pub window_seconds: u64,
}

/// API server configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ApiConfig {
    /// Port for HTTP API server
    #[serde(default = "default_api_port")]
    pub port: u16,

    /// Path to static files (frontend)
    #[serde(default = "default_static_path")]
    pub static_path: PathBuf,
}

impl Default for HftConfig {
    fn default() -> Self {
        Self {
            min_volume_24h: default_min_volume(),
            opportunity_threshold_bps: default_threshold(),
            window_seconds: default_window_seconds(),
        }
    }
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            port: default_api_port(),
            static_path: default_static_path(),
        }
    }
}

fn default_min_volume() -> f64 {
    1_000_000.0
}

fn default_threshold() -> i64 {
    250_000 // 0.25% in FixedPoint8
}

fn default_window_seconds() -> u64 {
    120 // 2 minutes
}

fn default_api_port() -> u16 {
    5000
}

fn default_static_path() -> PathBuf {
    PathBuf::from("/root/arbitrageR/reference/frontend")
}

impl Config {
    /// Load configuration from config.toml file
    ///
    /// If the file doesn't exist, returns default configuration.
    /// # Errors
    /// Returns error if file exists but cannot be parsed.
    pub fn load() -> Result<Self, ConfigError> {
        let config_path =
            std::env::var("CONFIG_PATH").unwrap_or_else(|_| "config.toml".to_string());

        match std::fs::read_to_string(&config_path) {
            Ok(contents) => {
                let config: Config = toml::from_str(&contents)
                    .map_err(|e| ConfigError::ParseError(e.to_string()))?;
                Ok(config)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // File not found - use defaults
                Ok(Config::default())
            }
            Err(e) => Err(ConfigError::IoError(e)),
        }
    }

    /// Get opportunity threshold as FixedPoint8 raw value
    #[inline(always)]
    pub fn opportunity_threshold_raw(&self) -> i64 {
        self.hft.opportunity_threshold_bps
    }
}

/// Configuration loading errors
#[derive(Debug)]
pub enum ConfigError {
    /// IO error reading file
    IoError(std::io::Error),
    /// Parse error (invalid TOML)
    ParseError(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::IoError(e) => write!(f, "Failed to read config file: {}", e),
            ConfigError::ParseError(e) => write!(f, "Failed to parse config: {}", e),
        }
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ConfigError::IoError(e) => Some(e),
            ConfigError::ParseError(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.hft.min_volume_24h, 1_000_000.0);
        assert_eq!(config.hft.opportunity_threshold_bps, 250_000);
        assert_eq!(config.api.port, 5000);
        assert_eq!(
            config.api.static_path,
            PathBuf::from("/root/arbitrageR/reference/frontend")
        );
    }

    #[test]
    fn test_opportunity_threshold_raw() {
        let config = Config::default();
        assert_eq!(config.opportunity_threshold_raw(), 250_000);
    }
}
