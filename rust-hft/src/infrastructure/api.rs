//! API Server (Cold Path)
//!
//! Serves dashboard static files and provides REST API for screener stats.
//! Accesses ThresholdTracker via shared state.

use axum::{
    extract::State,
    routing::get,
    Json, Router,
};
use serde::Serialize;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;

use crate::hot_path::{ScreenerStats, ThresholdTracker};
use crate::infrastructure::metrics::MetricsCollector;
use crate::HftError;

/// System status information
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemStatusDto {
    pub is_connected: bool,
    pub latency_ms: u64,
    pub active_symbols: usize,
    pub binance_connected: bool,
    pub bybit_connected: bool,
}

/// DTO for screener stats (matches store.js expectation)
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenerDto {
    pub symbol: String,
    pub current_spread: f64,
    pub spread_range: f64,
    pub hits: u64,
    pub est_half_life: f64,
    pub is_spread_na: bool,
}

/// Dashboard response DTO - combines system status and screener data
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardDto {
    pub system: SystemStatusDto,
    pub screener: Vec<ScreenerDto>,
}

impl From<ScreenerStats> for ScreenerDto {
    fn from(stats: ScreenerStats) -> Self {
        Self {
            symbol: stats.symbol.as_str().to_string(),
            current_spread: stats.current_spread.to_f64(),
            spread_range: stats.spread_range.to_f64(),
            hits: stats.hits,
            est_half_life: 0.0, // TODO: Implement half-life calculation
            is_spread_na: !stats.is_valid,
        }
    }
}

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub tracker: Arc<RwLock<ThresholdTracker>>,
    pub metrics: Arc<MetricsCollector>,
}

/// Start the API server
pub async fn start_server(
    tracker: Arc<RwLock<ThresholdTracker>>,
    metrics: Arc<MetricsCollector>,
    port: u16
) -> Result<(), HftError> {
    let state = AppState { tracker, metrics };

    // Static files service (from reference/frontend)
    // TODO: Use config for static path (Phase 6.5)
    let static_files = ServeDir::new("/root/arbitrageR/reference/frontend");

    let app = Router::new()
        // API Endpoints
        .route("/api/dashboard/stats", get(get_dashboard_stats))
        .route("/api/screener/stats", get(get_screener_stats))
        
        // Static files fallback
        .fallback_service(static_files)
        
        // Middleware
        .layer(CorsLayer::permissive())
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("API Server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await
        .map_err(|e| HftError::Io(e))?;
        
    axum::serve(listener, app).await
        .map_err(|e| HftError::Io(e))?;

    Ok(())
}

/// Handler for /api/dashboard/stats
/// Returns combined system status and screener data
async fn get_dashboard_stats(
    State(state): State<AppState>
) -> Json<DashboardDto> {
    // Note: Using write lock because get_all_stats needs to evict old entries
    // This is acceptable because API is cold path
    let mut tracker = state.tracker.write().await;
    let stats = tracker.get_all_stats();
    let active_symbols = stats.len();
    
    let screeners: Vec<ScreenerDto> = stats
        .into_iter()
        .map(ScreenerDto::from)
        .collect();
    
    // Get real metrics from collector
    let metrics_snapshot = state.metrics.snapshot();
    
    let system = SystemStatusDto {
        is_connected: state.metrics.is_connected(),
        latency_ms: state.metrics.latency_ms(),
        active_symbols,
        binance_connected: metrics_snapshot.binance_connected,
        bybit_connected: metrics_snapshot.bybit_connected,
    };
    
    Json(DashboardDto {
        system,
        screener: screeners,
    })
}

/// Handler for /api/screener/stats
/// Returns screener data only (backward compatibility)
async fn get_screener_stats(
    State(state): State<AppState>
) -> Json<Vec<ScreenerDto>> {
    // Note: Using write lock because get_all_stats needs to evict old entries
    let mut tracker = state.tracker.write().await;
    let stats = tracker.get_all_stats();
    
    let dtos: Vec<ScreenerDto> = stats
        .into_iter()
        .map(ScreenerDto::from)
        .collect();
        
    Json(dtos)
}
