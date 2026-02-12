//! Centralized file-based logging system
//!
//! Writes logs to files in logs/ directory, separated by log type:
//! - logs/main.log - General application logs
//! - logs/error.log - Error and warning logs only
//! - logs/ws.log - WebSocket connection logs
//! - logs/api.log - API server logs
//! - logs/exchange.log - Exchange-specific logs

use std::fs;
use std::path::Path;
use tracing::Level;
use tracing_appender::non_blocking::{NonBlocking, WorkerGuard};
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{
    layer::{Layer, SubscriberExt},
    util::SubscriberInitExt,
    EnvFilter,
};

/// Initialize centralized file logging
///
/// Creates logs/ directory and sets up file appenders for different log types.
/// Returns WorkerGuard which must be kept alive for the duration of the program.
pub fn init_logging() -> Vec<WorkerGuard> {
    // Create logs directory
    let logs_dir = Path::new("logs");
    if !logs_dir.exists() {
        fs::create_dir_all(logs_dir).expect("Failed to create logs directory");
    }

    // Create subdirectories for each log type
    let log_types = ["main", "error", "ws", "api", "exchange"];
    for log_type in &log_types {
        let dir = logs_dir.join(log_type);
        if !dir.exists() {
            fs::create_dir_all(&dir).expect("Failed to create log subdirectory");
        }
    }

    let mut guards = Vec::new();

    // Main log - all logs
    let (main_appender, main_guard) = create_appender("logs/main", "main");
    guards.push(main_guard);

    // Error log - ERROR and WARN only
    let (error_appender, error_guard) = create_appender("logs/error", "error");
    guards.push(error_guard);

    // WebSocket log - WS-related logs
    let (ws_appender, ws_guard) = create_appender("logs/ws", "ws");
    guards.push(ws_guard);

    // API log - API server logs
    let (api_appender, api_guard) = create_appender("logs/api", "api");
    guards.push(api_guard);

    // Exchange log - Exchange client logs
    let (exchange_appender, exchange_guard) = create_appender("logs/exchange", "exchange");
    guards.push(exchange_guard);

    // Create layers with filters
    let main_layer = tracing_subscriber::fmt::layer()
        .with_writer(main_appender)
        .with_ansi(false)
        .with_target(true)
        .with_level(true)
        .with_thread_ids(true)
        .with_thread_names(true)
        .json();

    let error_layer = tracing_subscriber::fmt::layer()
        .with_writer(error_appender)
        .with_ansi(false)
        .with_target(true)
        .with_level(true)
        .with_filter(tracing_subscriber::filter::LevelFilter::WARN);

    let ws_layer = tracing_subscriber::fmt::layer()
        .with_writer(ws_appender)
        .with_ansi(false)
        .with_target(true)
        .with_level(true)
        .with_filter(tracing_subscriber::filter::filter_fn(|metadata| {
            metadata.target().contains("ws")
                || metadata.target().contains("connection")
                || metadata.target().contains("websocket")
        }));

    let api_layer = tracing_subscriber::fmt::layer()
        .with_writer(api_appender)
        .with_ansi(false)
        .with_target(true)
        .with_level(true)
        .with_filter(tracing_subscriber::filter::filter_fn(|metadata| {
            metadata.target().contains("api") || metadata.target().contains("server")
        }));

    let exchange_layer = tracing_subscriber::fmt::layer()
        .with_writer(exchange_appender)
        .with_ansi(false)
        .with_target(true)
        .with_level(true)
        .with_filter(tracing_subscriber::filter::filter_fn(|metadata| {
            metadata.target().contains("exchange")
                || metadata.target().contains("binance")
                || metadata.target().contains("bybit")
        }));

    // Console layer for development
    let console_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_level(true);

    // Initialize subscriber with all layers
    tracing_subscriber::registry()
        .with(EnvFilter::new("info"))
        .with(main_layer)
        .with(error_layer)
        .with(ws_layer)
        .with(api_layer)
        .with(exchange_layer)
        .with(console_layer)
        .init();

    tracing::info!("Logging system initialized. Log files in logs/ directory");

    guards
}

/// Create a rolling file appender
fn create_appender(dir: &str, name: &str) -> (NonBlocking, WorkerGuard) {
    let appender = RollingFileAppender::new(Rotation::DAILY, dir, name);

    let (non_blocking, guard) = tracing_appender::non_blocking(appender);

    (non_blocking, guard)
}

/// Log macro helpers for specific log types
#[macro_export]
macro_rules! log_ws {
    ($level:expr, $($arg:tt)+) => {
        tracing::event!(target: "ws", $level, $($arg)+)
    };
}

#[macro_export]
macro_rules! log_api {
    ($level:expr, $($arg:tt)+) => {
        tracing::event!(target: "api", $level, $($arg)+)
    };
}

#[macro_export]
macro_rules! log_exchange {
    ($level:expr, $($arg:tt)+) => {
        tracing::event!(target: "exchange", $level, $($arg)+)
    };
}

#[macro_export]
macro_rules! log_main {
    ($level:expr, $($arg:tt)+) => {
        tracing::event!(target: "main", $level, $($arg)+)
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_directory_creation() {
        let test_dir = Path::new("logs_test");
        if test_dir.exists() {
            fs::remove_dir_all(test_dir).ok();
        }

        fs::create_dir_all(test_dir.join("main")).unwrap();
        assert!(test_dir.join("main").exists());

        fs::remove_dir_all(test_dir).ok();
    }
}
