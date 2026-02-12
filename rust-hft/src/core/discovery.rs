//! Symbol Discovery (Cold Path)
//!
//! Fetches liquid trading pairs from exchange REST APIs.
//! Filters by 24h volume to find high-liquidity symbols.
//! Called once at startup - NOT in hot path.

use crate::core::Symbol;
use crate::exchanges::Exchange;
use serde::Deserialize;
use std::time::Duration;

/// Minimum 24h volume in USDT to include symbol
pub const DEFAULT_MIN_VOLUME: f64 = 1_000_000.0;

/// Symbol information from exchange
#[derive(Debug, Clone)]
pub struct DiscoveredSymbol {
    pub symbol: Symbol,
    pub exchange: Exchange,
    pub volume_24h: f64,
    pub base_asset: String,
    pub quote_asset: String,
}

/// Symbol discovery client
pub struct SymbolDiscovery {
    client: reqwest::Client,
    min_volume: f64,
}

impl SymbolDiscovery {
    /// Create new discovery client
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .user_agent("rust-hft/0.1")
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
            min_volume: DEFAULT_MIN_VOLUME,
        }
    }

    /// Create with custom minimum volume
    pub fn with_min_volume(min_volume: f64) -> Self {
        let mut discovery = Self::new();
        discovery.min_volume = min_volume;
        discovery
    }

    /// Fetch liquid symbols from Binance Futures
    /// 
    /// API: GET https://fapi.binance.com/fapi/v1/ticker/24hr
    /// Returns all USDT-margined perpetuals with volume > min_volume
    pub async fn fetch_binance_liquid(&self) -> Result<Vec<DiscoveredSymbol>, DiscoveryError> {
        let url = "https://fapi.binance.com/fapi/v1/ticker/24hr";
        
        tracing::info!("Fetching Binance 24h tickers from {}", url);
        
        let response = self.client
            .get(url)
            .send()
            .await
            .map_err(|e| DiscoveryError::Network(e.to_string()))?;
        
        if !response.status().is_success() {
            return Err(DiscoveryError::Http(response.status().as_u16()));
        }
        
        let tickers: Vec<Binance24hTicker> = response
            .json()
            .await
            .map_err(|e| DiscoveryError::Parse(e.to_string()))?;
        
        tracing::info!("Received {} tickers from Binance", tickers.len());
        
        let symbols: Vec<DiscoveredSymbol> = tickers
            .into_iter()
            .filter(|t| t.quote_volume >= self.min_volume)
            .filter(|t| t.symbol.ends_with("USDT"))
            .filter_map(|t| {
                let symbol = Symbol::from_bytes(t.symbol.as_bytes())?;
                let (base, quote) = split_symbol_pair(&t.symbol)?;
                Some(DiscoveredSymbol {
                    symbol,
                    exchange: Exchange::Binance,
                    volume_24h: t.quote_volume,
                    base_asset: base.to_string(),
                    quote_asset: quote.to_string(),
                })
            })
            .collect();
        
        tracing::info!("Filtered to {} liquid symbols (volume >= {})", symbols.len(), self.min_volume);
        
        Ok(symbols)
    }

    /// Fetch liquid symbols from Bybit V5
    /// 
    /// API: GET https://api.bybit.com/v5/market/tickers?category=linear
    pub async fn fetch_bybit_liquid(&self) -> Result<Vec<DiscoveredSymbol>, DiscoveryError> {
        let url = "https://api.bybit.com/v5/market/tickers?category=linear";
        
        tracing::info!("Fetching Bybit tickers from {}", url);
        
        let response = self.client
            .get(url)
            .send()
            .await
            .map_err(|e| DiscoveryError::Network(e.to_string()))?;
        
        if !response.status().is_success() {
            return Err(DiscoveryError::Http(response.status().as_u16()));
        }
        
        let bybit_response: BybitTickersResponse = response
            .json()
            .await
            .map_err(|e| DiscoveryError::Parse(e.to_string()))?;
        
        if bybit_response.ret_code != 0 {
            return Err(DiscoveryError::Api(bybit_response.ret_msg));
        }
        
        let tickers = bybit_response.result.list;
        tracing::info!("Received {} tickers from Bybit", tickers.len());
        
        let symbols: Vec<DiscoveredSymbol> = tickers
            .into_iter()
            .filter(|t| {
                t.volume_24h.parse::<f64>().unwrap_or(0.0) * t.last_price.parse::<f64>().unwrap_or(0.0)
                    >= self.min_volume
            })
            .filter(|t| t.symbol.ends_with("USDT"))
            .filter_map(|t| {
                let symbol = Symbol::from_bytes(t.symbol.as_bytes())?;
                let (base, quote) = split_symbol_pair(&t.symbol)?;
                let volume = t.volume_24h.parse::<f64>().unwrap_or(0.0) 
                    * t.last_price.parse::<f64>().unwrap_or(0.0);
                Some(DiscoveredSymbol {
                    symbol,
                    exchange: Exchange::Bybit,
                    volume_24h: volume,
                    base_asset: base.to_string(),
                    quote_asset: quote.to_string(),
                })
            })
            .collect();
        
        tracing::info!("Filtered to {} liquid symbols from Bybit", symbols.len());
        
        Ok(symbols)
    }

    /// Fetch and merge symbols from all exchanges
    /// Returns unique symbols sorted by combined volume
    pub async fn fetch_all_liquid(&self) -> Result<Vec<DiscoveredSymbol>, DiscoveryError> {
        let (binance_result, bybit_result) = tokio::join!(
            self.fetch_binance_liquid(),
            self.fetch_bybit_liquid()
        );
        
        let mut all_symbols: Vec<DiscoveredSymbol> = Vec::new();
        
        if let Ok(binance) = binance_result {
            all_symbols.extend(binance);
        }
        
        if let Ok(bybit) = bybit_result {
            all_symbols.extend(bybit);
        }
        
        if all_symbols.is_empty() {
            return Err(DiscoveryError::NoSymbols);
        }
        
        // Sort by volume descending
        all_symbols.sort_by(|a, b| {
            b.volume_24h.partial_cmp(&a.volume_24h).unwrap_or(std::cmp::Ordering::Equal)
        });
        
        // Deduplicate by symbol (keep highest volume)
        let mut seen = std::collections::HashSet::new();
        all_symbols.retain(|s| seen.insert(s.symbol));
        
        Ok(all_symbols)
    }

    /// Fetch symbol names only (for registration before parsing)
    /// Returns unique USDT symbol names sorted by volume
    pub async fn fetch_symbol_names(&self) -> Result<Vec<String>, DiscoveryError> {
        let (binance_result, bybit_result) = tokio::join!(
            self.fetch_binance_names(),
            self.fetch_bybit_names()
        );

        let mut all_names: Vec<(String, f64)> = Vec::new();

        if let Ok(binance) = binance_result {
            all_names.extend(binance);
        }

        if let Ok(bybit) = bybit_result {
            all_names.extend(bybit);
        }

        if all_names.is_empty() {
            return Err(DiscoveryError::NoSymbols);
        }

        // Sort by volume descending
        all_names.sort_by(|a, b| {
            b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)
        });

        // Deduplicate by name (keep highest volume)
        let mut seen = std::collections::HashSet::new();
        let names: Vec<String> = all_names
            .into_iter()
            .filter(|(name, _)| seen.insert(name.clone()))
            .map(|(name, _)| name)
            .collect();

        Ok(names)
    }

    /// Fetch Binance symbol names with volumes
    async fn fetch_binance_names(&self) -> Result<Vec<(String, f64)>, DiscoveryError> {
        let url = "https://fapi.binance.com/fapi/v1/ticker/24hr";

        let response = self.client
            .get(url)
            .send()
            .await
            .map_err(|e| DiscoveryError::Network(e.to_string()))?;

        if !response.status().is_success() {
            return Err(DiscoveryError::Http(response.status().as_u16()));
        }

        let tickers: Vec<Binance24hTicker> = response
            .json()
            .await
            .map_err(|e| DiscoveryError::Parse(e.to_string()))?;

        let names: Vec<(String, f64)> = tickers
            .into_iter()
            .filter(|t| t.quote_volume >= self.min_volume)
            .filter(|t| t.symbol.ends_with("USDT"))
            .map(|t| (t.symbol, t.quote_volume))
            .collect();

        Ok(names)
    }

    /// Fetch Bybit symbol names with volumes
    async fn fetch_bybit_names(&self) -> Result<Vec<(String, f64)>, DiscoveryError> {
        let url = "https://api.bybit.com/v5/market/tickers?category=linear";

        let response = self.client
            .get(url)
            .send()
            .await
            .map_err(|e| DiscoveryError::Network(e.to_string()))?;

        if !response.status().is_success() {
            return Err(DiscoveryError::Http(response.status().as_u16()));
        }

        let bybit_response: BybitTickersResponse = response
            .json()
            .await
            .map_err(|e| DiscoveryError::Parse(e.to_string()))?;

        if bybit_response.ret_code != 0 {
            return Err(DiscoveryError::Api(bybit_response.ret_msg));
        }

        let names: Vec<(String, f64)> = bybit_response.result.list
            .into_iter()
            .filter(|t| {
                let volume = t.volume_24h.parse::<f64>().unwrap_or(0.0)
                    * t.last_price.parse::<f64>().unwrap_or(0.0);
                volume >= self.min_volume
            })
            .filter(|t| t.symbol.ends_with("USDT"))
            .map(|t| {
                let volume = t.volume_24h.parse::<f64>().unwrap_or(0.0)
                    * t.last_price.parse::<f64>().unwrap_or(0.0);
                (t.symbol, volume)
            })
            .collect();

        Ok(names)
    }
}

impl Default for SymbolDiscovery {
    fn default() -> Self {
        Self::new()
    }
}

/// Split symbol like "BTCUSDT" into ("BTC", "USDT")
fn split_symbol_pair(symbol: &str) -> Option<(&str, &str)> {
    if symbol.ends_with("USDT") {
        let base = &symbol[..symbol.len() - 4];
        if !base.is_empty() {
            return Some((base, "USDT"));
        }
    }
    None
}

// === API Response Types ===

/// Binance 24h ticker response
#[derive(Debug, Deserialize)]
struct Binance24hTicker {
    symbol: String,
    #[serde(rename = "quoteVolume")]
    quote_volume: f64,
}

/// Bybit tickers response
#[derive(Debug, Deserialize)]
struct BybitTickersResponse {
    #[serde(rename = "retCode")]
    ret_code: i32,
    #[serde(rename = "retMsg")]
    ret_msg: String,
    result: BybitResult,
}

#[derive(Debug, Deserialize)]
struct BybitResult {
    list: Vec<BybitTicker>,
}

#[derive(Debug, Deserialize)]
struct BybitTicker {
    symbol: String,
    #[serde(rename = "volume24h")]
    volume_24h: String,
    #[serde(rename = "lastPrice")]
    last_price: String,
}

/// Discovery errors
#[derive(Debug, thiserror::Error)]
pub enum DiscoveryError {
    #[error("Network error: {0}")]
    Network(String),
    
    #[error("HTTP error: {0}")]
    Http(u16),
    
    #[error("Parse error: {0}")]
    Parse(String),
    
    #[error("API error: {0}")]
    Api(String),
    
    #[error("No symbols found")]
    NoSymbols,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_symbol_pair() {
        assert_eq!(split_symbol_pair("BTCUSDT"), Some(("BTC", "USDT")));
        assert_eq!(split_symbol_pair("ETHUSDT"), Some(("ETH", "USDT")));
        assert_eq!(split_symbol_pair("1000PEPEUSDT"), Some(("1000PEPE", "USDT")));
        assert_eq!(split_symbol_pair("USDT"), None);
        assert_eq!(split_symbol_pair("BTC"), None);
    }

    #[test]
    fn test_discovery_creation() {
        let discovery = SymbolDiscovery::new();
        assert_eq!(discovery.min_volume, DEFAULT_MIN_VOLUME);
        
        let discovery = SymbolDiscovery::with_min_volume(5_000_000.0);
        assert_eq!(discovery.min_volume, 5_000_000.0);
    }

    #[test]
    fn test_binance_ticker_deserialize() {
        let json = r#"{"symbol":"BTCUSDT","quoteVolume":15000000000.0}"#;
        let ticker: Binance24hTicker = serde_json::from_str(json).unwrap();
        assert_eq!(ticker.symbol, "BTCUSDT");
        assert_eq!(ticker.quote_volume, 15000000000.0);
    }

    #[test]
    fn test_bybit_response_deserialize() {
        let json = r#"{
            "retCode": 0,
            "retMsg": "OK",
            "result": {
                "list": [
                    {
                        "symbol": "BTCUSDT",
                        "volume24h": "100000",
                        "lastPrice": "50000"
                    }
                ]
            }
        }"#;
        let response: BybitTickersResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.ret_code, 0);
        assert_eq!(response.result.list.len(), 1);
        assert_eq!(response.result.list[0].symbol, "BTCUSDT");
    }
}
