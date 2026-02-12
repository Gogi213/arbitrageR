//! API Server (Cold Path)
//!
//! Serves dashboard static files and provides REST API for screener stats.
//! Accesses ThresholdTracker via shared state.

use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;

use crate::core::Symbol;
use crate::hot_path::{ScreenerStats, ThresholdTracker};
use crate::HftError;

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub tracker: Arc<RwLock<ThresholdTracker>>,
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

/// Start the API server
pub async fn start_server(
    tracker: Arc<RwLock<ThresholdTracker>>,
    port: u16
) -> Result<(), HftError> {
    let state = AppState { tracker };

    // Static files service (from reference/frontend)
    // Using absolute path to ensure it works from any CWD
    let static_files = ServeDir::new("/root/arbitrageR/reference/frontend");

    let app = Router::new()
        // API Endpoints
        .route("/api/screener/stats", get(get_screener_stats))
        .route("/api/paper/stats", get(get_paper_stats)) // Stub for store.js compatibility
        
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

/// Handler for /api/screener/stats
async fn get_screener_stats(
    State(state): State<AppState>
) -> Json<Vec<ScreenerDto>> {
    let tracker = state.tracker.read().await;
    let stats = tracker.get_all_stats();
    
    let dtos: Vec<ScreenerDto> = stats
        .into_iter()
        .map(ScreenerDto::from)
        .collect();
        
    Json(dtos)
}

/// Stub handler for /api/paper/stats (to prevent store.js errors)
async fn get_paper_stats() -> Json<Vec<()>> {
    Json(vec![]) // Empty bots list
}
