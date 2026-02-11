//! WebSocket clients for real-time market data

pub mod connection;
pub mod ping;
pub mod pool;
pub mod subscription;

pub use connection::{WebSocketConnection, ConnectionState, WebSocketError};
pub use ping::{PingHandler, ConnectionMonitor, HeartbeatManager, ConnectionHealth};
pub use pool::{ConnectionPool, ConnectionConfig, ConnectionId, PoolStats};
