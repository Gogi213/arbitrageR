//! Infrastructure - cold path only
//!
//! This module contains non-latency-critical code:
//! - Logging and metrics
//! - Configuration management
//! - Health monitoring
//! - Graceful shutdown

pub mod config;
pub mod health;
pub mod logging;
pub mod metrics;
pub mod pool;
pub mod ring_buffer;

pub use pool::{ObjectPool, ByteBufferPool, MessageBufferPool};
pub use ring_buffer::RingBuffer;
